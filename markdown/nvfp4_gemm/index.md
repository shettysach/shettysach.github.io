---
title: NVFP4 GEMM
subtitle: Batched block-scaled GEMM using CuTe DSL
date: 2026-03-10
tags: [ GPU Kernels, GEMM, Optimization, CuTe DSL, CUDA ]
anchors: true
draft: true
---

# NVFP4 GEMM

This article aims to go over the batched block-scaled GEMM example for CuTe DSL,
and how I used it for the second round of the GPUMODE competition.

---

## Problem to be optimized

You can find the problem description on the [leaderboard page](https://www.gpumode.com/v2/leaderboard/597).


### GEMM

$$
C[x,w] = \sum_{y=0}^{K-1} A[x,y];B[y,w]
$$

* GEMM stands for **GE**neral **M**atrix–**M**atrix multiplication.
* $x$ is the index for the row dimension of $A$ and $C$, $w$ is the index for the column dimension of $B$ and $C$, and $y$ is the reduction  dimension.
* We multiply a matrix $A$ of shape $M \times K$ by a matrix $B$ of shape $K \times N$
  to produce a matrix $C$ of shape $M \times N$.

### Batched GEMM

$$
C[x,w,z] = \sum_{y=0}^{K-1} A[x,y,z];B[y,w,z]
$$

* In batched GEMM, we do $L$ independent GEMM computations, where $L$ is the number of batches / size of the batch dimension.
* $z$ is the index for the batch dimension. For each fixed $z$, we perform an independent GEMM.
* $A$ is a matrix of shape $M \times K \times L$, while $B$ is a matrix of shape $K \times N \times L$, and
  $C$ is a matrix of shape $M \times N \times L$.

### Batched block-scaled GEMM

$$
C[x,w,z] ;=; \sum_{y=0}^{K-1} A[x,y,z];\mathrm{SFA}[x,y,z]; B[y,w,z];\mathrm{SFB}[y,w,z]
$$

* In batched block-scaled GEMM, each value is paired with a scale factor —
  $A[x,y,z]$ is scaled by $\mathrm{SFA}[x,y,z]$, while $B[y,w,z]$ is scaled by $\mathrm{SFB}[y,w,z]$.
* $A$ and $\mathrm{SFA}$ are matrices of shape $M \times K \times L$, while
  $B$ and $\mathrm{SFB}$ are matrices of shape $K \times N \times L$, and
  $C$ is a matrix of shape $M \times N \times L$.

###  Representation in memory

- `a`, `b`, `sfa`, `sfb` and `c` are Torch tensors, passed as CuTe tensor views / pointers.
- `a` and `b` are of FP4 dtype, particularly [NVFP4](https://developer.nvidia.com/blog/introducing-nvfp4-for-efficient-and-accurate-low-precision-inference/), while `sfa` and `sfb` are of FP8 dtype. 
- The tensors are in $K$-major order / row-major order.
