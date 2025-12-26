---
title: NVFP4 Batched GEMV
subtitle: My submission for the first round of the GPUMODE Nvfp4 competition 
tags: [ CuTe DSL ]
draft: true
---

# NVFP4 Batched GEMV

This article details my submission for the [GPUMODE nvfp4_gemv leaderboard](https://www.gpumode.com/v2/leaderboard/595?tab=rankings).

---

## Batched block-scaled GEMV  

You can find the problem description on the leaderboard page.
The kernel computes a batched block‑scaled General Matrix-Vector multiplication (GEMV). 

In mathematical notation,

$$
C[x,0,z] \;=\; \sum_{y=0}^{K-1} \Bigl(A[x,y,z]\;\mathrm{SFA}[x,y,z]\Bigr)\; \Bigl(B[y,0,z]\;\mathrm{SFB}[y,0,z]\Bigr)
$$

- $A$ and $SFA$ are matrices of shape $M \times K \times L$, while $B$ and $SFB$ are vectors of shape $K \times 1 \times L$, and 
$C$ is a vector of shape $M \times 1 \times L$ 
- $M$ is the row dimension, $K$ is the column dimension and $L$ is the batch dimension
- Each value is paired with a scale factor - 
$A[x,y,z]$ is scaled by $\mathrm{SFA}[x,y,z]$, while $B[y,0,z]$ is scaled by $\mathrm{SFB}[y,0,z]$
- For each output element $C[x, 0, z]$ (row $x$, column fixed to $0$ as $N=1$, batch $z$), it performs a dot product over the column dimension, with per‑block scale factors applied to both operands. 

## CuTe DSL

<!-- TODO: -->
>TODO: 
>Write about Cute DSL. Move memory explanation here?

[CuTe DSL](https://docs.nvidia.com/cutlass/media/docs/pythonDSL/cute_dsl.html)

In memory,

- `a`, `b`, `sfa`, `sfb` and `c` are tensors
- `a` and `b` are of fp4 dtype, while `sfa` and `sfb` are of fp8 dtype 
- `a` and `sfa` are layed out as `(m, k, l)`, while `b` and `sfb` are layed out as `(128, k, l)`, permuted and padded for easier tiling
- `c` is of fp16 dtype and layed out as `(m, 1, l)`

---

## Reference kernel

I based my implementation on the CuTe template kernel, `template_cute.py`, that can be found [here](https://github.com/gpu-mode/reference-kernels/blob/main/problems/nvidia/nvfp4_gemv/template_cute.py). Here is a breakdown of the file.

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
It performs the batched block-scaled GEMV operation.

1. Each CTA handles a tile of the output `c` - 128 rows x 1 column (`mma_tiler_mnk[:2]`) for one batch $l$
2. Each thread (`tidx`) is responsible for 1 output element `c[x,0,z]` within that tile
3. CuTe’s `local_tile` creates matching tiled views of `a, b, sfa, sfb, c`, so the right chunks line up in memory
4. The kernel reduces over $K$ in chunks of 64 elements (`mma_tiler_mnk[2]`): load fp4 and fp8, convert to fp32, then accumulate the scaled products
5. After all $K$ tiles are processed, it casts fp32 to fp16 and stores the result to `c`


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

---

## Improvements

### Parallelism over $K$

The reference kernel implements

#### Parallelism over $M$ (output rows)
- At the block level, different blocks (`bidx`) each take a chunk of 128 rows to compute.
- Within each block, different threads (`tidx`) compute different rows within that 128-row chunk.

####  Parallelism over $L$ (batch dimension):
- At the block level, different blocks along `bidz` handle different batch indices $l$.
- Each block computes outputs for a single $l$, and many `bidz` blocks run in parallel.

```py
# Inside `kernel`:
bidx, bidy, bidz = cute.arch.block_idx()  # bidx selects which 128-row chunk of M, bidy is 0 (GEMV), bidz handles batch
tidx, _, _ = cute.arch.thread_idx()       # tidx selects the row within that chunk

...
# Each thread picks exactly one output element C[m, 0, l]:
tCgC = gC_mnl[tidx, None, bidx, bidy, bidz]

...
k_tile_cnt = gA_mkl.layout[3].shape
for k_tile in range(k_tile_cnt):
```

Launch parameters 

```py
# Inside `my_kernel`:
kernel(...).launch(
    grid=(
        cute.ceil_div(c_tensor.shape[0], 128),  # how many 128-row blocks cover M
        1,                                      # N is 1 (GEMV)
        c_tensor.shape[2],                      # one block-slice per batch L
    ),
    block=[threads_per_cta, 1, 1],              # 128 threads per block
    cluster=(1, 1, 1),
)
```

Changes applied to the kernel 

#### Parallelism over $K$ (the reduction dimension)
- In the reference kernel, one thread computes the full dot product over all $K$ elements for its output.
- In the new kernel, we split that work across multiple threads using `tidy`. Each thread handles a subset of the $K$ tiles in a strided loop:

#### Parallelism over $L$ inside a block (via `threads_l`)
- Instead of having one block handle only one batch index $l$, we let a block cover multiple $l$ values using `tidz`:

```python
# Inside `kernel`:
bidx, bidy, bidz = arch.block_idx()
tidx, tidy, tidz = arch.thread_idx()

l_block = bidz * threads_l + tidz

...
tCgC = gC_mnl[tidx, None, bidx, bidy, l_block]

...
for k_block in range(tidy, k_tile_cnt, threads_k):
```

New launch parameters 
- Now threads handle each dimension. They need to adhere to these limits. 
- Maximum threads per block = 1024. So $threads_k \times threads_m \times threads_l \leq 1024$.
- Max dimensions per block, $threads_m \leq 1024, threads_k \leq 1024, threads_l \leq 64$ 

```py
# Inside `my_kernel`:
kernel(...).launch(
    grid=(
           ceil_div(size[0], threads_m),
           1,
           ceil_div(size[3], threads_l),  # to cover L batches, each block handles `threads_l` indices
       ),
       block=[threads_k, threads_m, threads_l],
       cluster=(1, 1, 1),
   )
   ```

### Reduction of partial sums (combining the $K$ work)

Once we split the $K$ loop across `threads_k` threads (via `tidy`), each thread computes a **partial sum** for the same output element. We then need to reduce across `tidy`to get the final dot product for that output. There are two ways we do this.

#### I. Shared-memory reduction

Shared memory (SMEM) is a small, fast memory that is shared by all threads in the same block (CTA).  
Threads can write their partial results to SMEM, synchronize with `__syncthreads()` in CUDA / `arch.sync_threads()` in CuTe DSL, 
and then have one or more threads read from SMEM to sum everything up.  

1. Each thread writes its partial sum to a shared-memory buffer indexed by `(tidx, tidy, tidz)`

    ```py
    allocator = cutlass.utils.SmemAllocator()
    layout = cute.make_layout((threads_m, threads_k + 1, threads_l))
    res = allocator.allocate_tensor(Float16, layout)
    ...
    for k_block in range(tidy, k_tile_cnt, threads_k):
        ...
        for i in cutlass.range_constexpr(0, k_tile):
          res[tidx, tidy, tidz] = tABrAB[i] * tSFrSF[i]
    ```

1. Block sync is used to make sure all partials are visible

    ```py
    arch.sync_threads()
    ```

2. Then a single thread per output (here `tidy == 0`) loops over `tidy` and adds them up, and stores the result

    ```py
    if tidy == 0:
        out = cute.zeros_like(tCgC, Float32)
        for i in cutlass.range_constexpr(threads_k):
            out += res[tidx, i, tidz]
        tCgC.store(out.to(c_dtype))
    ```

#### II. Warp shuffle reduction 

On NVIDIA GPUs, a warp is a group of 32 threads that run together. 
These 32 threads execute the same instructions in lockstep. 
We can use warp operations like shuffle to let those 32 threads share values quickly without SMEM for the reduction.

1. Each thread keeps its partial sum in a register

    ```python
    res = Float32(0)
    ...
    for k_block in range(tidy, k_tile_cnt, threads_k):
        ...
        for i in cutlass.range_constexpr(0, k_tile):
          res = tABrAB[i] * tSFrSF[i]
    ```

1. Warp shuffle butterfly ops are used to sum values across the `tidy` dimension

    ```python
    offset = threads_k >> 1
    while offset > 0:
        res += arch.shuffle_sync_bfly(res, offset, threads_k)
        offset >>= 1
    ```

1. After the shuffle reduction, lane 0 (`tidy == 0`) has the total and writes it out

    ```python
    if tidy == 0:
        out = scalar_to_ssa(res, acc_dtype)
        tCgC.store(out.to(c_dtype))
    ```

This avoids shared memory and avoids `sync_threads`, so it’s usually faster as long as the participating threads are in one warp.
