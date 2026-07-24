---
title: NVFP4 GEMV
subtitle: My submission for the first round of the GPUMODE NVFP4 competition
date: 2026-01-10
tags: [ GPU Kernels, Optimization, CuTe DSL, CUDA ]
---

# NVFP4 GEMV

<!--toc:start-->
- [NVFP4 GEMV](#nvfp4-gemv)
  - [Problem to be optimized](#problem-to-be-optimized)
    - [GEMV](#gemv)
    - [Batched GEMV](#batched-gemv)
    - [Batched block-scaled GEMV](#batched-block-scaled-gemv)
    - [Representation in memory](#representation-in-memory)
  - [CuTe DSL](#cute-dsl)
  - [Reference kernel](#reference-kernel)
    - [Setup](#setup)
    - [CuTe kernel](#cute-kernel)
    - [Host-side launcher](#host-side-launcher)
    - [Kernel compilation and caching](#kernel-compilation-and-caching)
    - [Entry point](#entry-point)
  - [Optimizations](#optimizations)
    - [I. Restructuring the multiplication and accumulation](#i-restructuring-the-multiplication-and-accumulation)
    - [II. Parallelism over dimensions](#ii-parallelism-over-dimensions)
      - [Parallelism over $M$ (output rows)](#parallelism-over-m-output-rows)
      - [Parallelism over $L$ (batch dimension)](#parallelism-over-l-batch-dimension)
      - [Parallelism over $K$ (the reduction dimension)](#parallelism-over-k-the-reduction-dimension)
      - [Parallelism over $L$ inside a block (via `threads_l`)](#parallelism-over-l-inside-a-block-via-threadsl)
    - [III. Reduction of partial sums (combining the $K$ work)](#iii-reduction-of-partial-sums-combining-the-k-work)
      - [SMEM reduction](#smem-reduction)
      - [Warp shuffle reduction](#warp-shuffle-reduction)
    - [IV. Using FP16 Fused-Multiply-Accumulate](#iv-using-fp16-fused-multiply-accumulate)
    - [V. Shape specific tuning](#v-shape-specific-tuning)
  - [Ideas that didn't pay off](#ideas-that-didnt-pay-off)
  - [Flaws](#flaws)
  - [Credits and thank you](#credits-and-thank-you)
<!--toc:end-->

This article details my submission for the [GPUMODE nvfp4_gemv leaderboard](https://www.gpumode.com/v2/leaderboard/595?tab=rankings) 
and the various techniques I used to improve over the reference kernel to make it faster, but also what it lacked.

---

## Problem to be optimized

You can find the problem description on the [leaderboard page](https://www.gpumode.com/v2/leaderboard/595).
The kernel computes a batched block‑scaled GEMV. 

### GEMV

$$
C[x, 0] = \sum_{y=0}^{K-1} A[x,y]\;B[y, 0]
$$

- GEMV stands for **GE**neral **M**atrix–**V**ector multiplication.
- $x$ is the index for the row dimension of $A$ and $C$. 
The reduction happens along $y$, which is the index of 
the column dimension of $A$ and the row dimension of $B$.
- We multiply a matrix $A$ of shape $M \times K$ by a vector $B$ of shape $K \times 1$ 
to produce another vector $C$ of shape $M \times 1$.

### Batched GEMV

$$
C[x,0,z] = \sum_{y=0}^{K-1} A[x,y,z]\;B[y,0,z]
$$

- In batched GEMV, we do $L$ independent GEMV computations, where $L$ is the number of batches / size of batch dimension. 
- $z$ is the index for the batch dimension. For each fixed $z$, we perform an independent GEMV.
- $A$ is a matrix of shape $M \times K \times L$, while $B$ is a vector of shape $K \times 1 \times L$, and 
$C$ is a vector of shape $M \times 1 \times L$.

### Batched block-scaled GEMV

$$
C[x,0,z] \;=\; \sum_{y=0}^{K-1} A[x,y,z]\;\mathrm{SFA}[x,y,z]\; B[y,0,z]\;\mathrm{SFB}[y,0,z]
$$

- In batched block-scaled GEMV, each value is paired with a scale factor - 
$A[x,y,z]$ is scaled by $\mathrm{SFA}[x,y,z]$, while $B[y,0,z]$ is scaled by $\mathrm{SFB}[y,0,z]$.
- $A$ and $SFA$ are matrices of shape $M \times K \times L$, while $B$ and $SFB$ are vectors of shape $K \times 1 \times L$, and 
$C$ is a vector of shape $M \times 1 \times L$.

###  Representation in memory

- `a`, `b`, `sfa`, `sfb` and `c` are Torch tensors, passed as CuTe tensor views / pointers.
- `a` and `b` are of FP4 dtype, particularly [NVFP4](https://developer.nvidia.com/blog/introducing-nvfp4-for-efficient-and-accurate-low-precision-inference/), while `sfa` and `sfb` are of FP8 dtype. 
- The tensors are in $K$-major order / row-major order.
- `a` and `sfa` are given the shape `(m, k, l)` with strides `(k, 1, m * k)`, 
while `b` and `sfb` are given the shape `(128, k, l)` with strides `(k, 1, 128 * k)`, 
`n` padded to 128 for easier tiling.
- `c` is of FP16 dtype and is given the shape `(m, 1, l)` with strides `(1, 1, l)`. 
  
There are 3 shapes to target, `n` is always 1 since GEMV
- `m: 7168, k: 16384, l: 1`
- `m: 4096, k: 7168, l: 8`
- `m: 7168, k: 2048, l: 4`

## CuTe DSL

[CuTe DSL](https://docs.nvidia.com/cutlass/latest/media/docs/pythonDSL/cute_dsl.html) 
for Python is a high‑level, Python front end over NVIDIA’s CUTLASS/CuTe tensor core infrastructure 
that lets you describe GPU kernels in terms of tiled tensors, layouts, and copy/compute primitives, and also low-level optimizations when needed. 
The tiling abstraction seems to be the direction kernel frameworks are headed, and since there was a template file using the DSL, 
I chose to use CuTe DSL for the problem.

## Reference kernel

I based my implementation on the CuTe DSL template kernel, `template_cute.py`, that can be found [here](https://github.com/gpu-mode/reference-kernels/blob/main/problems/nvidia/nvfp4_gemv/template_cute.py). Here is a breakdown of the file.

### Setup

This sets up the required imports, the data types of the input, output, and scale factor matrices, and parameters such as the tile size and number of threads per CUDA thread block / CTA (Cooperative Thread Array). 

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

1. Each CTA handles a tile of the output `c` - 128 rows x 1 column (`mma_tiler_mnk[:2]`) for one batch $l$.
2. Each thread (`tidx`) is responsible for 1 output element `c[x,0,z]` within that tile.
3. CuTe DSL’s `local_tile` creates matching tiled views of `a, b, sfa, sfb, c`, so the right chunks line up in memory.
4. The kernel reduces over $K$ in chunks of 64 elements (`mma_tiler_mnk[2]`) - load FP4 and FP8, convert to FP32, then accumulate the scaled products.
5. After all $K$ tiles are processed, it casts FP32 to FP16 and stores the result to `c`.


```py
# The CuTe DSL reference implementation for NVFP4 block-scaled GEMV
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

        # Store the converted values to RMEM CuTe DSL tensors
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

`compile_kernel` JIT-compiles `my_kernel` once and stores the compiled result in `_compiled_kernel_cache`. On later calls, it returns the cached compiled function so you don’t pay compilation overhead again.

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

The doc comment explains it pretty well.

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

## Optimizations

### I. Restructuring the multiplication and accumulation

In the reference kernel, each iteration does this.
- Load FP4/FP8 values from global memory.
- Convert them to FP32.
- Store the converted vectors into separate (RMEM) tensors (`tArA`, `tBrB`, `tArSFA`, `tBrSFB`). 
- Inside the inner loop, repeatedly loads from those RMEM tensors to multiply 4 values and accumulate.

```py
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
```

In the new version, we keep the same load and convert step, but instead of creating 4 RMEM tensors and storing into them, we
compute the two products, one with the data and another with the scale factors, `tABrAB = a_vec * b_vec` and `tSFrSF = sfa_vec * sfb_vec`. 
Then the inner loop becomes a multiply-accumulate.

```py
# Load NVFP4 or FP8 values from global memory
a_val_nvfp4 = tAgA.load()
b_val_nvfp4 = tBgB.load()
sfa_val_fp8 = tAgSFA.load()
sfb_val_fp8 = tBgSFB.load()

# Convert loaded values to float32 for computation (FFMA)
a_vec = a_val_nvfp4.to(cutlass.Float32)
b_vec = b_val_nvfp4.to(cutlass.Float32)
sfa_vec = sfa_val_fp8.to(cutlass.Float32)
sfb_vec = sfb_val_fp8.to(cutlass.Float32)

# multiplication happens in the registers 
tABrAB = a_vec * b_vec
tSFrSF = sfa_vec * sfb_vec

# Iterate over SF vector tiles and compute the scale&matmul accumulation
for i in cutlass.range_constexpr(mma_tiler_mnk[2]):
    res += tABrAB[i] * tSFrSF[i]
```

The `tABrAB`, `tSFrSF` and the `*_vec` variables, are `TensorSSA` and the computation happens in the registers.
Here is a Jupyter notebook from the CUTLASS repo, [tensorssa.ipynb](https://github.com/NVIDIA/cutlass/blob/main/examples/python/CuTeDSL/notebooks/tensorssa.ipynb) that explains the `TensorSSA` abstraction and how to use them. 
                       
This provides a small speedup and makes it easier for later optimizations.

### II. Parallelism over dimensions

The reference kernel implements

#### Parallelism over $M$ (output rows)
- At the block level, different blocks (`bidx`) each take a chunk of 128 rows to compute.
- Within each block, different threads (`tidx`) compute different rows within that 128-row chunk.

####  Parallelism over $L$ (batch dimension)
- At the block level, different blocks along `bidz` handle different batch indices $l$.
- Each block computes outputs for a single $l$, and many `bidz` blocks run in parallel.

Here `bidy` always remains 0 due to GEMV.

```py
# Inside `kernel`:
bidx, bidy, bidz = cute.arch.block_idx()  # bidx selects which 128-row chunk of M, bidz handles batch
tidx, _, _ = cute.arch.thread_idx()       # tidx selects the row within that chunk

...
# Each thread picks exactly one output element C[m, 0, l]:
tCgC = gC_mnl[tidx, None, bidx, bidy, bidz]

...
k_tile_cnt = gA_mkl.layout[3].shape
for k_tile in range(k_tile_cnt):
```

The reference kernel then uses these launch parameters 

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

Based on [Simon's blogpost](https://veitner.bearblog.dev/nvfp4-gemv-improved/), I made these improvements over the kernel.

#### Parallelism over $K$ (the reduction dimension)
- In the reference kernel, one thread computes the full dot product over all $K$ elements for its output.
- In the new kernel, we split that work across multiple threads using `tidy`. Each thread handles a subset of the $K$ tiles in a loop.

#### Parallelism over $L$ inside a block (via `threads_l`)
- Instead of having one block handle only one batch index $l$, we let a block cover multiple $l$ values using `tidz`.

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

New launch parameters. Now threads handle each dimension. They need to adhere to these limits - 
- Maximum threads per block = 1024. So $threads_k \times threads_m \times threads_l \leq 1024$.
- Max dimensions per block, $threads_m \leq 1024, threads_k \leq 1024, threads_l \leq 64$. 

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

### III. Reduction of partial sums (combining the $K$ work)

Once we split the $K$ loop across `threads_k` threads (via `tidy`), each thread computes a partial sum for the same output element. We then need to reduce across `tidy` to get the final dot product for that output. There are two ways we do this.

#### SMEM reduction

Shared memory (SMEM) is a small, fast memory that is shared by all threads in the same block (CTA).  
Threads can write their partial results to SMEM, synchronize with `__syncthreads()` in CUDA / `arch.sync_threads()` in CuTe DSL, 
and then have one or more threads read from SMEM to sum everything up. This is one of the approaches used in the blogpost above.  

- Each thread writes its partial sum to a shared-memory buffer indexed by `(tidx, tidy, tidz)`.

    ```py
    allocator = cutlass.utils.SmemAllocator()
    layout = cute.make_layout(
        (threads_m, threads_k, threads_l),
        stride=(threads_k * threads_l, 1, threads_k), # K-major
    )
    res = allocator.allocate_tensor(Float16, layout)
    r = Float32(0)

    ...
    for k_block in range(tidy, k_tile_cnt, threads_k):
        ...
        for i in cutlass.range_constexpr(0, k_tile):
          r += tABrAB[i] * tSFrSF[i]
    ```

- Block sync is used to make sure all partials are visible.

    ```py
    res[tidx, tidy, tidz] = r
    arch.sync_threads()
    ```

- Then a single thread per output (`tidy == 0`) loops over `tidy` and adds them up, and stores the result.

    ```py
    if tidy == 0:
        out = cute.zeros_like(tCgC, Float32)
        for i in cutlass.range_constexpr(threads_k):
            out += res[tidx, i, tidz]
        tCgC.store(out.to(c_dtype))
    ```

#### Warp shuffle reduction 

On NVIDIA GPUs, a warp is a group of 32 threads that run together. 
These 32 threads execute the same instructions in lockstep. 
We can use warp operations like shuffle to let those 32 threads share values quickly without SMEM for the reduction.

- Each thread keeps its partial sum in a register.

    ```python
    res = Float32(0)
    ...
    for k_block in range(tidy, k_tile_cnt, threads_k):
        ...
        for i in cutlass.range_constexpr(0, 128):
          res += tABrAB[i] * tSFrSF[i]
    ```

- Warp shuffle butterfly ops are used to sum values across the `tidy` dimension.

    ```python
    offset = threads_k >> 1
    while offset > 0:
        res += arch.shuffle_sync_bfly(res, offset, threads_k)
        offset >>= 1 # right shift assign, not monadic bind 
    ```

- After the shuffle reduction, lane 0 (`tidy == 0`) has the total and writes it out.

    ```python
    if tidy == 0:
        out = scalar_to_ssa(res, acc_dtype)
        tCgC.store(out.to(c_dtype))
    ```

    where `scalar_to_ssa` is this helper, taken from the [FlashAttention repo](https://github.com/Dao-AILab/flash-attention/blob/58fe37fba6b07ac0aa6e88a94d68f8378c901028/flash_attn/cute/utils.py#L843).

    ```python
    @cute.jit
    def scalar_to_ssa(a: cute.Numeric, dtype) -> cute.TensorSSA:
        vec = cute.make_rmem_tensor(1, dtype)
        vec[0] = a
        return vec.load()
    ```

This avoids shared memory and avoids `sync_threads`, so it’s usually faster as long as the participating threads are in one warp.

Shape 1 `(7168, 16384, 1)` was faster with the SMEM reduction while the other 2 were faster with the warp shuffle reduction.

### IV. Using FP16 Fused-Multiply-Accumulate

This is based entirely on [Simon's blogpost](https://veitner.bearblog.dev/demystifying-numeric-conversions-in-cutedsl/).
The B200 has 32-bit registers. As of yet we have been converting FP4 and FP8 to FP32, then performing 1 FP32 calculation on these registers at a time.
However, we can actually convert them to FP16 and then perform 2 FP16 calculations on a single 32-bit register using 2 16-bit lanes, but we have to take precision errors into consideration.

We can convert NVFP4 vectors `a_vec` and `b_vec` to FP16 using the `.to(Float16)` method in the `TensorSSA` class. 
However, to convert FP8 vectors `sfa_vec` and `sfb_vec`, we have to use these low-level PTX instructions from [Simon's blogpost](https://veitner.bearblog.dev/demystifying-numeric-conversions-in-cutedsl/), which goes more into depth. 
Since I always use it on vectors of length 128 , I could simplify it and only use the part for vectors of length divisible by 8.    

```python
@dsl_user_op
def cvt_f8e4m3_f16_intr(vec_f8e4m3, length, *, loc=None, ip=None):
    src_pos = 0
    vec_src_i8 = builtin.unrealized_conversion_cast(
        [ir.VectorType.get([length], Int8.mlir_type, loc=loc)],
        [vec_f8e4m3],
        loc=loc,
        ip=ip,
    )
    vec_i8x8_type = ir.VectorType.get([8], Int8.mlir_type, loc=loc)
    vec_dst_type = ir.VectorType.get([length], Float16.mlir_type, loc=loc)
    vec_dst = llvm.mlir_zero(vec_dst_type, loc=loc, ip=ip)

    num_vec8 = length // 8
    for _ in range(num_vec8):
        vec_f8e4m3x8 = vector.extract_strided_slice(
            vec_i8x8_type, vec_src_i8, [src_pos], [8], [1], loc=loc, ip=ip
        )
        vec_f16x8 = cvt_f8e4m3x8_to_f16x8(vec_f8e4m3x8, loc=loc, ip=ip)
        vec_dst = vector.insert_strided_slice(vec_f16x8, vec_dst, [src_pos], [1], loc=loc, ip=ip)
        src_pos += 8
        length -= 8

    return vec_dst

# Convert 8 float8e4m3 values to 8 float16 values
@dsl_user_op
def cvt_f8e4m3x8_to_f16x8(src_vec8, *, loc=None, ip=None):
    # Split into two i32 values instead of using i64
    vec_i32x2_type = ir.VectorType.get([2], Int32.mlir_type, loc=loc)
    src_i32x2 = llvm.bitcast(vec_i32x2_type, src_vec8, loc=loc, ip=ip)
    src_lo = llvm.extractelement(src_i32x2, arith.constant(Int32.mlir_type, 0), loc=loc, ip=ip)
    src_hi = llvm.extractelement(src_i32x2, arith.constant(Int32.mlir_type, 1), loc=loc, ip=ip)

    # Process lower 4 bytes (4 fp8 values)
    rst_lo_i32x2 = llvm.inline_asm(
        llvm.StructType.get_literal([T.i32(), T.i32()]),
        [src_lo],
        """{\n\t
            .reg .b16 h0, h1;\n\t
            mov.b32 {h0, h1}, $2;\n\t
            cvt.rn.f16x2.e4m3x2 $0, h0;\n\t
            cvt.rn.f16x2.e4m3x2 $1, h1;\n\t
        }""",
        "=r,=r,r",
    )

    # Process upper 4 bytes (4 fp8 values)
    rst_hi_i32x2 = llvm.inline_asm(
        llvm.StructType.get_literal([T.i32(), T.i32()]),
        [src_hi],
        """{\n\t
            .reg .b16 h0, h1;\n\t
            mov.b32 {h0, h1}, $2;\n\t
            cvt.rn.f16x2.e4m3x2 $0, h0;\n\t
            cvt.rn.f16x2.e4m3x2 $1, h1;\n\t
        }""",
        "=r,=r,r",
    )

    res0 = llvm.extractvalue(T.i32(), rst_lo_i32x2, [0])
    res1 = llvm.extractvalue(T.i32(), rst_lo_i32x2, [1])
    res2 = llvm.extractvalue(T.i32(), rst_hi_i32x2, [0])
    res3 = llvm.extractvalue(T.i32(), rst_hi_i32x2, [1])

    vec_i32x4_type = ir.VectorType.get([4], Int32.mlir_type, loc=loc)
    vec_i32x4 = vector.from_elements(vec_i32x4_type, [res0, res1, res2, res3], loc=loc, ip=ip)
    vec_f16x8_type = ir.VectorType.get([8], Float16.mlir_type, loc=loc)
    vec_f16x8 = llvm.bitcast(vec_f16x8_type, vec_i32x4, loc=loc, ip=ip)
    return vec_f16x8
```

Converting the vectors to FP16.

```python
a_vec = tAgA.load().to(Float16)
b_vec = tBgB.load().to(Float16)
sfa_vec = TensorSSA(cvt_f8e4m3_f16_intr(tAgSFA.load(), k_tile), k_tile, Float16)
sfb_vec = TensorSSA(cvt_f8e4m3_f16_intr(tBgSFB.load(), k_tile), k_tile, Float16)

# Also FP16 now
tABrAB = a_vec * b_vec
tSFrSF = sfa_vec * sfb_vec
```

Now, we can perform 2 16-bit Fused-Multiply-Accumulate (FMA) operations at once in a 32-bit register.
`fma.rn.f16x2` is the PTX instruction to perform the 2 FMAs and round to the nearest representable number. 

```python
@dsl_user_op
def fma_f16x2(
    a: tuple[Float16, Float16],
    b: tuple[Float16, Float16],
    c: tuple[Float16, Float16],
    *,
    loc=None,
    ip=None,
) -> tuple[Float16, Float16]:
    # Pack two Float16 values into vector<2xf16>
    vec_type = ir.VectorType.get([2], Float16.mlir_type, loc=loc)

    vec_a = vector.from_elements(
        vec_type,
        [a[0].ir_value(loc=loc, ip=ip), a[1].ir_value(loc=loc, ip=ip)],
        loc=loc,
        ip=ip,
    )
    vec_b = vector.from_elements(
        vec_type,
        [b[0].ir_value(loc=loc, ip=ip), b[1].ir_value(loc=loc, ip=ip)],
        loc=loc,
        ip=ip,
    )
    vec_c = vector.from_elements(
        vec_type,
        [c[0].ir_value(loc=loc, ip=ip), c[1].ir_value(loc=loc, ip=ip)],
        loc=loc,
        ip=ip,
    )

    # Bitcast to i32 for PTX (f16x2 is packed into 32 bits)
    a_i32 = llvm.bitcast(Int32.mlir_type, vec_a, loc=loc, ip=ip)
    b_i32 = llvm.bitcast(Int32.mlir_type, vec_b, loc=loc, ip=ip)
    c_i32 = llvm.bitcast(Int32.mlir_type, vec_c, loc=loc, ip=ip)

    result_i32 = llvm.inline_asm(
        Int32.mlir_type,
        [a_i32, b_i32, c_i32],
        "fma.rn.f16x2 $0, $1, $2, $3;",
        "=r,r,r,r",
        has_side_effects=False,
        is_align_stack=False,
        asm_dialect=llvm.AsmDialect.AD_ATT,
        loc=loc,
        ip=ip,
    )

    # Bitcast back to vector<2xf16>
    vec_result = llvm.bitcast(vec_type, result_i32, loc=loc, ip=ip)

    # Extract results
    result0 = Float16(vector.extract(vec_result, [], [0], loc=loc, ip=ip))
    result1 = Float16(vector.extract(vec_result, [], [1], loc=loc, ip=ip))

    return result0, result1
```

Now `r0` and `r1` hold the running partial FMA sums for the corresponding alternate even (`i`) and odd (`i+1`) elements. 
Each iteration performs `r0 += tABrAB[i] * tSFrSF[i]` and `r1 += tABrAB[i+1] * tSFrSF[i+1]`.

```python
for i in cutlass.range_constexpr(0, 128, 2):
    r0, r1 = fma_f16x2(
        (tABrAB[i], tABrAB[i + 1]),
        (tSFrSF[i], tSFrSF[i + 1]),
        (r0, r1),
    )
```

Now, to combine the partial sums, for shapes 1 and 3, I just combined the two FP16s 
into a single FP16 after the loop with no precision errors.

- For the shape 1, 
    ```python
    res = allocator.allocate_tensor(Float16, layout)
    r0, r1 = Float16(0), Float16(0)

    for k_block in range(tidy, k_tile_cnt, threads_k):
        ...
        for i in cutlass.range_constexpr(0, 128, 2):
            r0, r1 = fma_f16x2(...)

    res[tidx, tidy, tidz] = r0 + r1
    arch.sync_threads()

    ...
    # SMEM reduction
    ```

- For the shape 3, 
    ```python
    r0, r1 = Float16(0), Float16(0)

    for k_block in range(tidy, k_tile_cnt, threads_k):
        ...
        for i in cutlass.range_constexpr(0, 128, 2):
            r0, r1 = fma_f16x2(...)

    res = r0 + r1 # Also a Float16

    ...
    # Warp shuffle reduction
    ```

But for shape 2, due to precision error accumulation, I had to use an FP32 accumulator 
which I had to update in each iteration of the outer loop.

```python
res = Float32(0)
for k_block in range(tidy, k_tile_cnt, threads_k):
    ...
    r0, r1 = Float16(0), Float16(0)
    for i in cutlass.range_constexpr(0, 128, 2):
        r0, r1 = fma_f16x2(...)

    res += r0 + r1

...
# Warp shuffle reduction
```

### V. Shape specific tuning

The most straightforward optimization. Tuning the number of threads per dimension for each of the 3 shapes.
I tuned the tile sizes after applying each of the above optimizations and restructuring the file, 
settling for these values at the end.

```py
def select_threads(m: int, k: int, l: int) -> tuple[int, int, int]:
    if (m, k, l) == (7168, 16384, 1):
        return (64, 16, 1)

    if (m, k, l) == (4096, 7168, 8):
        return (64, 4, 4)

    if (m, k, l) == (7168, 2048, 4):
        return (256, 4, 1)

    return (128, 8, 1)
```

Based on the threads, I chose the tile sizes like this.
I experimented with k_tile = 256, but found 128 to be the best in the end.

```py
m_tile = threads_m
k_tile = 128
mnk_tile = (m_tile, 1, k_tile)
```

---

## Ideas that didn't pay off

Manually prefetching tiles didn’t help. However, like I'll elaborate in the next section, using copy primitives and pipelining provided by CuTe DSL such as [Copy Atom](https://docs.nvidia.com/cutlass/media/docs/pythonDSL/cute_dsl_api/cute.html#cutlass.cute.CopyAtom) likely could have helped.

Using 2 16-bit lane addition and multiplication using the PTX instructions `add.f16x2` and `mul.rn.f16x2`, similar to the FMA optimization, 
didn't provide a speedup probably because the compiler already optimized this. I also tried reducing `r0` and `r1` separately, then combining them at the end, but it either introduced small precision differences or didn’t improve performance.

I also experimented with manual loop unrolling, but it didn’t help. CuTe DSL unrolls the fixed-size loops well. Similarly, I tried tweaking compilation options (optimization level, explicit GPU architecture, and PTXAS flags like `--maxrregcount`), but the defaults were already optimal.

## Flaws

The improved kernel is still far from optimal. CuTe DSL gives you building blocks for Tensor Core MMA via the `tcgen05` set for Blackwell, pipelines for data movement (so loads and compute overlap), and shared-memory tiling patterns that make sure threads cooperate and reuse data efficiently. 
In my version, I’m still mostly doing a manual load, convert, compute loop and only using CuTe DSL for tiling and launching, so I’m not taking full advantage of CuTe DSL and its higher-level features. 

The blogpost [tcgen05 for dummies, from gau-nernst's blog](https://gau-nernst.github.io/tcgen05/) goes into detail about writing efficient kernels with the `tcgen05` set. The CUTLASS repo also provides [CuTe DSL examples for Blackwell GPUs](https://github.com/NVIDIA/cutlass/tree/main/examples/python/CuTeDSL/blackwell) for various GEMM variants.
I did go on to use these features in the kernels for the next rounds, 
modifying the [dense_blockscaled_gemm_persistent.py](https://github.com/NVIDIA/cutlass/blob/main/examples/python/CuTeDSL/blackwell/dense_blockscaled_gemm_persistent.py) example.

Apart from that, even with this manual approach, I should still be able to improve things like memory access patterns and register pressure/occupancy.
A combination of higher-level CuTe DSL features (like copy primitives, swizzled layouts), together with this manual approach could have helped, but I wasn’t familiar enough with CuTe DSL at the time to use them effectively.

Regarding the article itself, it would be much better if I had specified the time taken / speedups after each optimization, but
I only got the idea to write much later, so I didn’t record them while I was iterating. 
I could try checking my older submissions, but those intermediate solutions were messy and had ideas scattered across files, 
so the speedup numbers per optimization wouldn't be very accurate.

## Credits and thank you

Again, I give huge credit to [Simon's blog](https://veitner.bearblog.dev/blog/), 
for helping me get started with CuTe DSL and the problem, and for learning about ways to optimize.
I'd also like to thank GPUMODE and NVIDIA for organizing the competition.

This was my first submission to a kernel optimization contest and my first work log.
You can find my submission at the [leaderboard page](https://www.gpumode.com/leaderboard/595?tab=rankings) 
under the name swanbomb_ (25.989μs). I would recommend checking out the faster CuTe DSL solutions to see their approaches too.
Thank you for reading.
