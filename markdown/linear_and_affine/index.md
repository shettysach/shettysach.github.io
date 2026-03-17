---
title: Linear and Affine
subtitle: In types and transformations
date: 2026-03-17
tags: [ Math, Type Theory, Linear Algebra, Vector Spaces ]
---

# Linear and Affine

---

While reading about the basics of type theory and rasterization in 3D graphics, I came across 2 pairs of concepts - 
linear and affine types, and linear and affine transformations.
Both pairs of concepts make a clear distinction between linear and affine, with affine having a bit more freedom than linear, 
but I wanted to get a clear understanding of their relation and a possible common origin.

## Types

Linear and affine type systems tell you how many times a value may be used.

A linear type must be used exactly once. This means it cannot be left unused, or used more than once.
In Linear Haskell, a function with a linear arrow `%1 ->` guarantees that its argument is consumed exactly once by the function body[^1].

```haskell
consumeFile :: Handle %1 -> IO ()
consumeFile handle = hClose handle  -- must use handle exactly once
```

If you tried to use `handle` twice, or not at all, the compiler would reject it.

An affine type relaxes this slightly. A value must be used at most once. You can use it once, or drop it entirely, but you cannot use it twice. 
Rust's ownership system is essentially an affine type system. When you move a value, the original binding is consumed and cannot be used again.

```rust
let s = String::from("hello");
let t = s;         // s is moved into t
println!("{}", s); // error: s has been moved
```

The compiler doesn't require that `s` be used. You could let it drop at the end of the scope, but you can't use it after it has already been moved.

Linear and affine types are useful for managing resources like file handles, network connections, and memory, ensuring they are always closed or freed, and never used after. The same guarantees also help the compiler skip reference counting and garbage collection entirely, helping in performance too.

 <!--
 TODO: Elaborate and correct more 
--> 

## Transformations

Linear and affine transformations come from linear algebra and vector spaces, 
and are applied in fields such as 3D graphics, for transforming and projecting 3D objects.

In linear algebra, a transformation is just a function. 
It takes a point in vector space and maps it to another point in a vector space. 
A linear transformation, a linear function and a linear map refer to the same thing in linear algebra.
What makes a transformation linear or affine is a constraint on how that mapping is allowed to behave.

A linear transformation is one that preserves the origin and scales uniformly with its input. 
Formally, it must satisfy two conditions - it should be [additive](https://en.wikipedia.org/wiki/Additive_map), 
and should scale linearly with its input[^2]:

$$
\begin{align*}
f(u + v) &= f(u) + f(v) \\
f(\alpha v) &= \alpha f(v)
\end{align*}
$$

Together, these force $f(0) = 0$. The origin must map to itself[^3]. Every linear function or transformation can be written in the form:

$$f(x) = Ax$$

where $A$ is a matrix, and $Ax$ represents [matrix-vector multiplication](/nvfp4_gemv/index.html#gemv).
In vector space, this covers rotation, scaling, and reflection, but not translation.

An affine transformation relaxes this by allowing a translation on top, meaning the origin can move.
The general form is just a linear transformation with a shift added:

$$f(x) = Ax + b$$

where $b$ is a vector, and $Ax + b$ represents vector addition. Now $f(0) = b$ instead of $0$, so objects can actually move through space, not just rotate or scale in place.
Thus, rendering 3D graphics mostly relies on affine transformations, as translation is important for various pipelines
such as rasterization.

## The Common Origin and Intuition

Both pairs of concepts trace back to the same underlying idea, which comes from Girard's linear logic[^4].

In classical logic, you can freely use a premise as many times as you want or ignore it entirely. Girard refers to these two rules as - 
- __Contraction__, which lets you duplicate an assumption, using it more than once.
- __Weakening__, which lets you discard an assumption, ignoring it entirely.

Linear logic rejects both. If you have a resource, you must use it and you must use it only once. Affine logic rejects only contraction and allows weakening. If you have a resource you can use it at most once, and are allowed to discard it without using. 

You can see how linear and affine types are directly analogous to this, where the resources are instances of the types. The transformations also take from this concept.
 Consider a function written as a power series:

$$f(x) = a_0 + a_1 x + a_2 x^2 + \cdots$$

Each term corresponds to a different degree of use of the input $x$.
- The $a_1 x$ term uses $x$ exactly once. This is the linear part 
- The $a_0$ term ignores $x$ entirely. This is weakening 
- The higher-degree terms like $a_2 x^2$ use $x$ more than once. This is contraction. 

A linear function, in Girard's sense, keeps only the $a_1 x$ term, and the input is used exactly once. This is similar to the usage of a linear type, and to the $f(x) = Ax$ form of linear transformations. 

An affine function allows the $a_0$ term too. The input may be used once, or ignored entirely, similar to the usage of an affine type, and is similar to the $f(x) = Ax + b$ form of affine transformations. 

## Closing notes

I wrote this post because I couldn't immediately understand the association of the two different concepts. 
One dealt with the consumption of resources, and the other dealt with transformations of shapes in a 3D space.
This article isn't too deep into the math and I've only given a surface level view of various topics. 
You are encouraged to check the resources in the footnotes.

---

__Footnotes__

[^1]: [6.4.22. Linear types — Glasgow Haskell Compiler User's Guide](https://ghc.gitlab.haskell.org/ghc/doc/users_guide/exts/linear_types.html)
[^2]: A function that scales linearly with its input is a [homogeneous function](https://en.wikipedia.org/wiki/Homogeneous_function) of order 1

[^3]: 
    A linear function in linear algebra and a linear function in calculus are [not the same](https://en.wikipedia.org/wiki/Linear_function). \
    In calculus, $f(x)=x+3$ can be called a linear function as it is a polynomial of degree 1, but in linear algebra, $f(x)=x+3$ cannot be called a linear function as $f(0) \neq{} 0$.

[^4]: [LINEAR LOGIC : ITS SYNTAX AND SEMANTICS - Jean-Yves Girard](https://girard.perso.math.cnrs.fr/Synsem.pdf) \
<https://en.wikipedia.org/wiki/Linear_logic>

__Other resources__

- [Benefits of linear types over affine types? - r/ProgrammingLanguage ](https://www.reddit.com/r/ProgrammingLanguages/comments/1e1o07f/benefits_of_linear_types_over_affine_types/)
- [Linear VS Affine - Lei Mao's Log Book](https://leimao.github.io/blog/Linear-VS-Affine/)
