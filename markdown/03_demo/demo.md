---
title: Demo
subtitle: A post to demonstrate the blog's features.
tags:
  - markdown
  - about
  - demo
  - test
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

You can create internal anchor links using `{#id}` syntax in headings.

* [Lists](#lists)
* [Mathematical Expressions](#mathematical-expressions)
* [Code Blocks](#code-blocks)

## Lists {#lists}

Lists are useful for outlining steps or concepts. Both unordered and ordered lists are supported.

### Unordered

- Clean formatting
- Support for nested items
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

### Rust Example

```rust
fn main() {
    println!("Hello, blog!");
}
```

### Python Example

```python
def greet():
    print("Hello, blog!")
```

## Links and Images

### Links

[Link to another article](../03_render/render.html)

Written as `[Link to another article](../03_render/render.html)`.

### Image

![dog](./dog.jpg)

Written as `![dog](./dog.jpg)`.

