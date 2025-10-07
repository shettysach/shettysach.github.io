---
title: Demonstration
subtitle: A post to demonstrate the blog's features.
tags:
  - Markdown
  - Blog
toc: true
---

# Demonstration

This article showcases the features supported by the blog engine, including formatting, code, math, and anchor links.

## Headings and Paragraphs

Organize content using headings and paragraphs. Markdown supports:

- _Emphasis_ using underscores or asterisks
- **Strong emphasis** for highlighting
- Inline `code` and $\text{math}$ for technical notations

> Blockquotes can be used for citations or remarks, clearly separated from main content.

## Anchors and Linking

You can create internal anchor links using `{#id}` syntax in headings. Eg - [Lists](#lists).

## Lists {#lists}

Lists are useful for outlining steps or concepts. Both unordered and ordered lists are supported.

### Unordered

- Clean formatting
  - Support for nested items
    - Can include **formatting**, `code`, and $math$
      - Can include **formatting**, `code`, and $math$

### Ordered

1. Write Markdown content
2. Embed metadata at the top
3. Use the renderer to generate static HTML

## Mathematical Expressions {#mathematical-expressions}

Mathematics can be rendered using LaTeX syntax.

### Inline Math

Euler's identity, written inline with `$e^{i\pi} + 1 = 0$`, renders as:  $e^{i\pi} + 1 = 0$

### Display Math

Use fenced code blocks with `latex` for display equations:

```latex
$$
\int_0^\infty e^{-x^2} \, dx = \frac{\sqrt{\pi}}{2}
$$
````

Which renders as:

$$
\int_0^\infty e^{-x^2} \, dx = \frac{\sqrt{\pi}}{2}
$$

## Code Blocks {#code-blocks}

Code blocks are highlighted using syntax-aware rendering. Specify the language for proper highlighting.


```rust
fn sum_of_squares(xs: &[i32]) -> i32 {
    xs.iter().map(|&x| x * x).sum()
}
```

```cpp
#include <type_traits>

template <int N>
struct Fib : std::integral_constant<long long, Fib<N - 1>::value + Fib<N - 2>::value> {};
template <>
struct Fib<1> : std::integral_constant<long long, 1> {};
template <>
struct Fib<0> : std::integral_constant<long long, 0> {};
```

```python
def evens_squared(nums: list[int]) -> list[int]:
    return [n * n for n in nums if n % 2 == 0]
```

```haskell
data Tree v = Leaf v | Node (Maybe (Tree v)) v (Maybe (Tree v))

invert :: Tree v -> Tree v
invert (Node l v r) = Node (invert <$> r) v (invert <$> l)
invert (Leaf v)     = Leaf v
```

## Links and Images {#links-and-images}

### Links

[Link to another article](../01_render/render.html)

Written as `[Link to another article](../03_render/render.html)`.

### Image

![apple](https://encrypted-tbn0.gstatic.com/images?q=tbn:ANd9GcQvh9e5WKA1njqGh-8xSgh52YDdepBiH5DqpA&s)

Written as `![apple](url to image)`.

