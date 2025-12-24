---
title: NVFP4 GEMV
subtitle: My submission for the first round of the GPUMODE Nvfp4 competition 
tags: [ CuTe DSL ]
hmin: 1
hmax: 6
---

# NVFP4 GEMV

## Problem description

You will implement a batched matrix-vector multiplication kernel optimized for NVIDIA B200.
To be explicit, you will be given a tuple of tensors:
```
(a, b, sfa, sfb, c)
```
where:
* `a` is M x K x L in K-major order in nvfp4(e2m1)
* `b` is 1 x K x L in K-major order in nvfp4(e2m1)
* `sfa` is M x (K // 16) x L in K-major order in fp8(e4m3fnuz)
* `sfb` is 1 x (K // 16) x L in K-major order in fp8(e4m3fnuz)
* `c` is M x 1 x L in fp16

Matrix sizes `M` is divisible by mma_tiler_mn[0] defined in the kernel, `K` is divisible by 64.
The ranking criteria is the geometric mean of the benchmark results.
For the grand price, your kernel will be evaluated against the speed of light analysis
and the solution closest to the speed of light will be awarded the grand price.
```
The speed of light analysis based on the max(FFMA math throughput, DRAM memory throughput) of B200 and tested under 1.5Ghz clock:
M K L time[us]
7168 16384 1 8.622
4096 7168 8 17.275
7168 2048 4 4.317
```

## Reference kernel

I iterated on the CuTe template kernel, `template_cute.py`, that can be found [here](https://github.com/gpu-mode/reference-kernels/blob/main/problems/nvidia/nvfp4_gemv/template_cute.py). Here is a breakdown of the file.

### Setup

This sets up the required imports, the data types of the input,output and scale factor matrices, and parameters such as the tile size and number of threads per CUDA thread block / CTA (Co-operative Thread Array). 

```py
from task import input_t, output_t

import cutlass
import cutlass.cute as cute
from cutlass.cute.runtime import make_ptr
import cutlass.utils.blockscaled_layout as blockscaled_utils

# Kernel configuration parameters
mma_tiler_mnk = (128, 1, 64)     # Tile sizes for M, N, K dimensions
ab_dtype = cutlass.Float4E2M1FN  # FP4 data type for A and B
sf_dtype = cutlass.Float8E4M3FN  # FP8 data type for scale factors
c_dtype = cutlass.Float16        # FP16 output type
sf_vec_size = 16                 # Scale factor block size (16 elements share one scale)
threads_per_cta = 128            # Number of threads per CUDA thread block


# Helper function for ceiling division
def ceil_div(a, b):
    return (a + b - 1) // b
```

### CuTe kernel

`kernel` is the main kernel function that runs on the device (B200 GPU), decorated with `@cute.kernel`.

At a high level, this kernel computes a block‑scaled GEMV. For each output element $C[m, 0, l]$ (row $m$, column fixed to $0$ as $N=1$, batch $l$), it performs a dot product over $K$, but with per‑block scale factors applied to both operands -

$$
C[m,0,l] = \sum_k (A[m,k,l]\cdot \mathrm{SFA}[m,k,l])\cdot(B[0,k,l]\cdot \mathrm{SFB}[0,k,l])
$$

1. Each CTA handles a tile of the output: 128 rows x 1 column (`mma_tiler_mnk[:2]`) for one batch $l$.  
2. Each thread (`tidx`) is responsible for 1 output element $C[m,0,l]$ within that tile.  
3. CuTe’s `local_tile` creates matching tiled views of `A, B, SFA, SFB, C`, so the right chunks line up in memory.  
4. The kernel reduces over $K$ in chunks of 64 elements (`mma_tiler_mnk[2]`): load `FP4` and `FP8`, convert to `FP32`, then accumulate the scaled products.  
5. After all $K$ tiles are processed, it casts `FP32 -> FP16` and stores the result to `C`.


```py
# The CuTe reference implementation for NVFP4 block-scaled GEMV
@cute.kernel
def kernel(
    mA_mkl: cute.Tensor,
    mB_nkl: cute.Tensor,
    mSFA_mkl: cute.Tensor,
    mSFB_nkl: cute.Tensor,
    mC_mnl: cute.Tensor,
):
    # Get CUDA block and thread indices
    bidx, bidy, bidz = cute.arch.block_idx()
    tidx, _, _ = cute.arch.thread_idx()

    # Extract the local tile for input matrix A (shape: [block_M, block_K, rest_M, rest_K, rest_L])
    gA_mkl = cute.local_tile(mA_mkl, cute.slice_(mma_tiler_mnk, (None, 0, None)), (None, None, None))
    # Extract the local tile for scale factor tensor for A (same shape as gA_mkl)
    # Here, block_M = (32, 4); block_K = (16, 4)
    gSFA_mkl = cute.local_tile(mSFA_mkl, cute.slice_(mma_tiler_mnk, (None, 0, None)), (None, None, None))
    # Extract the local tile for input matrix B (shape: [block_N, block_K, rest_N, rest_K, rest_L])
    gB_nkl = cute.local_tile(mB_nkl, cute.slice_(mma_tiler_mnk, (0, None, None)), (None, None, None))
    # Extract the local tile for scale factor tensor for B (same shape as gB_nkl)
    gSFB_nkl = cute.local_tile(mSFB_nkl, cute.slice_(mma_tiler_mnk, (0, None, None)), (None, None, None))
    # Extract the local tile for output matrix C (shape: [block_M, block_N, rest_M, rest_N, rest_L])
    gC_mnl = cute.local_tile(mC_mnl, cute.slice_(mma_tiler_mnk, (None, None, 0)), (None, None, None))

    # Select output element corresponding to this thread and block indices
    tCgC = gC_mnl[tidx, None, bidx, bidy, bidz]
    tCgC = cute.make_tensor(tCgC.iterator, 1)
    res = cute.zeros_like(tCgC, cutlass.Float32)

    # Get the number of k tiles (depth dimension) for the reduction loop
    k_tile_cnt = gA_mkl.layout[3].shape
    for k_tile in range(k_tile_cnt):
        tAgA = gA_mkl[tidx, None, bidx, k_tile, bidz]
        tBgB = gB_nkl[0, None, bidy, k_tile, bidz]
        tAgSFA = gSFA_mkl[tidx, None, bidx, k_tile, bidz]
        tBgSFB = gSFB_nkl[0, None, bidy, k_tile, bidz]

        tArA = cute.make_rmem_tensor_like(tAgA, cutlass.Float32)
        tBrB = cute.make_rmem_tensor_like(tBgB, cutlass.Float32)
        tArSFA = cute.make_rmem_tensor_like(tAgSFA, cutlass.Float32)
        tBrSFB = cute.make_rmem_tensor_like(tBgSFB, cutlass.Float32)

        # Load NVFP4 or FP8 values from global memory
        a_val_nvfp4 = tAgA.load()
        b_val_nvfp4 = tBgB.load()
        sfa_val_fp8 = tAgSFA.load()
        sfb_val_fp8 = tBgSFB.load()

        # Convert loaded values to float32 for computation (FFMA)
        a_val = a_val_nvfp4.to(cutlass.Float32)
        b_val = b_val_nvfp4.to(cutlass.Float32)
        sfa_val = sfa_val_fp8.to(cutlass.Float32)
        sfb_val = sfb_val_fp8.to(cutlass.Float32)

        # Store the converted values to RMEM CuTe tensors
        tArA.store(a_val)
        tBrB.store(b_val)
        tArSFA.store(sfa_val)
        tBrSFB.store(sfb_val)

        # Iterate over SF vector tiles and compute the scale&matmul accumulation
        for i in cutlass.range_constexpr(mma_tiler_mnk[2]):
            res += tArA[i] * tArSFA[i] * tBrB[i] * tBrSFB[i]

    # Store the final float16 result back to global memory
    tCgC.store(res.to(cutlass.Float16))
    return
```

### Host-side launcher

`my_kernel` is the host-side JIT wrapper - it runs on the CPU, but generates and launches the GPU kernel. It is decorated with `@cute.jit`. 

It takes raw device pointers (`a_ptr`,...) plus `problem_size = (m, n, k, l)` and constructs CuTe Tensor views (`a_tensor`,...) from those pointers by attaching the correct shapes, strides, and scale factor layouts.  

Launch parameters -
- `block`: Threads per CUDA block. Here it’s `128` threads working together.
- `grid`: Number of CUDA blocks to launch. Here it launches enough blocks to cover all $M$ rows, and one set per batch $L$.  
- `cluster`: Groups multiple blocks so they can cooperate. Here it’s `(1, 1, 1)`, so each block runs independently.


```py
@cute.jit
def my_kernel(
    a_ptr: cute.Pointer,
    b_ptr: cute.Pointer,
    sfa_ptr: cute.Pointer,
    sfb_ptr: cute.Pointer,
    c_ptr: cute.Pointer,
    problem_size: tuple,
):
    """
    Host-side JIT function to prepare tensors and launch GPU kernel.
    """

    # Create CuTe Tensors via pointer and problem size.
    a_tensor, b_tensor, sfa_tensor, sfb_tensor, c_tensor = ...

    # Compute grid dimensions
    # Grid is (M_blocks, 1, L) where:
    # - M_blocks = ceil(M / 128) to cover all output rows
    # - L = batch size
    grid = (
        cute.ceil_div(c_tensor.shape[0], 128),
        1,
        c_tensor.shape[2],
    )

    # Launch the CUDA kernel
    kernel(a_tensor, b_tensor, sfa_tensor, sfb_tensor, c_tensor).launch(
        grid=grid,
        block=[threads_per_cta, 1, 1],
        cluster=(1, 1, 1),
    )
    return
```

### Kernel compilation and caching

`compile_kernel` JIT-compiles my_kernel once and stores the compiled result in `_compiled_kernel_cache`. On later calls, it returns the cached compiled function so you don’t pay compilation overhead again.

It creates dummy CuTe pointers with the right dtypes and address space (global memory). These are only used to tell the compiler the argument types/layout expectations. It calls `cute.compile(...)` with those pointer types and a placeholder problem size `(0, 0, 0, 0)` to produce a callable compiled kernel.

```py
# Global cache for compiled kernel
_compiled_kernel_cache = None

def compile_kernel():
    """
    Compile the kernel once and cache it.
    This should be called before any timing measurements.

    Returns:
        The compiled kernel function
    """
    global _compiled_kernel_cache
    if _compiled_kernel_cache is not None:
        return _compiled_kernel_cache

    # Create CuTe pointers for A/B/C/SFA/SFB via torch tensor data pointer
    a_ptr = make_ptr(ab_dtype, 0, cute.AddressSpace.gmem, assumed_align=16)
    b_ptr = make_ptr(ab_dtype, 0, cute.AddressSpace.gmem, assumed_align=16)
    c_ptr = make_ptr(c_dtype, 0, cute.AddressSpace.gmem, assumed_align=16)
    sfa_ptr = make_ptr(sf_dtype, 0, cute.AddressSpace.gmem, assumed_align=32)
    sfb_ptr = make_ptr(sf_dtype, 0, cute.AddressSpace.gmem, assumed_align=32)

    # Compile the kernel
    _compiled_kernel_cache = cute.compile(my_kernel, a_ptr, b_ptr, sfa_ptr, sfb_ptr, c_ptr, (0, 0, 0, 0))
    return _compiled_kernel_cache
```

### Entry point

`custom_kernel` is the function the evaluation script looks for and calls to run the solution. It  bridges PyTorch to CuTe by turning each tensor’s `data_ptr()` into a CuTe pointer with the correct dtype and alignment. It also makes sure the kernel is already JIT-compiled, launches the compiled kernel with the runtime problem size, and returns the PyTorch output tensor `c` that was written in-place on the GPU.

```python
def custom_kernel(data: input_t) -> output_t:
    """
    Execute the block-scaled GEMV kernel.

    This is the main entry point called by the evaluation framework.
    It converts PyTorch tensors to CuTe tensors, launches the kernel,
    and returns the result.

    Args:
        data: Tuple of (a, b, sfa_cpu, sfb_cpu, c) PyTorch tensors
            a: [m, k, l] - Input matrix in float4e2m1fn
            b: [1, k, l] - Input vector in float4e2m1fn
            sfa_cpu: [m, k, l] - Scale factors in float8_e4m3fn
            sfb_cpu: [1, k, l] - Scale factors in float8_e4m3fn
            sfa_permuted: [32, 4, rest_m, 4, rest_k, l] - Scale factors in float8_e4m3fn
            sfb_permuted: [32, 4, rest_n, 4, rest_k, l] - Scale factors in float8_e4m3fn
            c: [m, 1, l] - Output vector in float16

    Returns:
        Output tensor c with computed GEMV results
    """
    a, b, _, _, sfa_permuted, sfb_permuted, c = data

    # Ensure kernel is compiled (will use cached version if available)
    # To avoid the compilation overhead, we compile the kernel once and cache it.
    compiled_func = compile_kernel()
    
    # Get dimensions from MxKxL layout
    # Create CuTe pointers for A/B/C/SFA/SFB via torch tensor data pointer
    a_ptr, b_ptr, sfa_ptr, sfb_ptr, c_ptr = ...

    # Execute the compiled kernel
    compiled_func(a_ptr, b_ptr, sfa_ptr, sfb_ptr, c_ptr, (m, n, k, l))

    return c
```
