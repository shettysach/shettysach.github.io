---
title: Rendering math and highlighting code
subtitle: How the blog renders math and highlights syntax, using static HTML and CSS.
tags:
    - SSG
    - Blog
    - Rust
    - LaTeX
---

# Rendering math and highlighting code

This article explains how the blog renders math and highlights syntax, using static HTML and CSS.
The relevant code for this section can be found [here](https://github.com/shettysach/shettysach.github.io/blob/main/src/syntex.rs).

---

## Rendering math { #math }

$$
\sum_{i=1}^{n} i^3 = \left( \frac{n(n+1)}{2} \right) ^2
$$

### LaTeX

LaTeX is a high-quality typesetting system; it includes features designed for the production of technical and scientific documentation. LaTeX is the de facto standard for the communication and publication of scientific documents. LaTeX is available as free software. [[The Latex Project]](https://www.latex-project.org/)

```latex
\sum_{i=1}^{n} i^3 = \left( \frac{n(n+1)}{2} \right) ^2
```
_LaTeX representation of the above equation_

However, web browsers and standard HTML cannot directly interpret LaTeX syntax. Therefore, the blog converts LaTeX into MathML Core.

### MathML

Mathematical Markup Language (MathML) is an XML-based language for describing mathematical notation.
MathML Core is a subset with increased implementation details based on rules from LaTeX and the Open Font Format. It is tailored for browsers and designed specifically to work well with other web standards including HTML, CSS, DOM, JavaScript.
[[MDN Web Docs]](https://developer.mozilla.org/en-US/docs/Web/MathML)

```html
<math display="block"><munderover><mo movablelimits="false">∑</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mrow><mi>n</mi></mrow></munderover><msup><mi>i</mi><mn>3</mn></msup><mo>=</mo><msup><mrow><mo stretchy="true">(</mo><mfrac><mrow><mi>n</mi><mo symmetric="false" stretchy="false">(</mo><mi>n</mi><mo>+</mo><mn>1</mn><mo symmetric="false" stretchy="false">)</mo></mrow><mrow><mn>2</mn></mrow></mfrac><mo stretchy="true">)</mo></mrow><mn>2</mn></msup></math>
```
_MathML representation of the above equation_

To convert from LaTeX to MathML, [`pulldown_cmark`](https://crates.io/crates/pulldown-cmark) events are used to identify math and code sections. When `pulldown-cmark` parses the markdown file, it detects the tags for both inline and block math expressions. Then, the LaTeX inside these sections, is converted to MathML using the [`pulldown_latex`](https://crates.io/crates/pulldown-latex) crate. The resultant MathML is streamed and the math is rendered. 

Sum of cubes of the first $n$ natural numbers

$$
\sum_{i=1}^{n} i^3 = \left( \frac{n(n+1)}{2} \right) ^2
$$

### Syntax for math sections

There are two types of math displays,

#### Inline display - $F(n) = n^2$

Written as

```latex
$F(n) = n^2$
```

#### Block display

$$
F(n) =
\begin{cases}
0 & \text{if } n = 0, \\
1 & \text{if } n = 1, \\
F(n-1) + F(n-2) & \text{if } n \geq 2.
\end{cases}
$$

written as
  
```latex
$$
F(n) =
\begin{cases}
0 & \text{if } n = 0, \\
1 & \text{if } n = 1, \\
F(n-1) + F(n-2) & \text{if } n \geq 2.
\end{cases}
$$
```
  
---

## Highlighting code { #code }

```rust
fn sum_of_cubes_lhs(n: usize) -> usize {
    (2..=n).map(|i| i * i * i).sum()
}

fn sum_of_cubes_rhs(n: usize) -> usize {
    let s = n * (n + 1) / 2;
    s * s
}
```

For syntax highlighting, the blog uses the [`autumnus`](https://crates.io/crates/autumnus) crate,
which uses [`Tree-sitter`](https://github.com/tree-sitter/tree-sitter) under the hood. 
Tree-sitter powers the highlighting in editors such as 
[Neovim](https://neovim.io/doc/user/treesitter.html) and [Zed](https://zed.dev/blog/syntax-aware-editing).

When `pulldown-cmark` identifies a codeblock,
`autumnus` generates HTML with CSS classes for different code elements
like keywords, variables and constants. This enables for syntax highlighting
through CSS classes, which makes it easy to have colours assigned to different elements.

```html
<pre class="athl"><code class="language-rust" translate="no" tabindex="0"><div class="line" data-line="1"><span class="keyword-function">fn</span> <span class="function">sum_of_cubes_lhs</span><span class="punctuation-bracket">(</span><span class="variable-parameter">n</span><span class="punctuation-delimiter">:</span> <span class="type-builtin">usize</span><span class="punctuation-bracket">)</span> <span class="punctuation-delimiter">-&gt;</span> <span class="type-builtin">usize</span> <span class="punctuation-bracket">&lbrace;</span>
</div><div class="line" data-line="2">    <span class="punctuation-bracket">(</span><span class="number">2</span><span class="operator">..=</span><span class="variable">n</span><span class="punctuation-bracket">)</span><span class="punctuation-delimiter">.</span><span class="function-call">map</span><span class="punctuation-bracket">(</span><span class="punctuation-bracket">|</span><span class="variable-parameter">i</span><span class="punctuation-bracket">|</span> <span class="variable">i</span> <span class="operator">*</span> <span class="variable">i</span> <span class="operator">*</span> <span class="variable">i</span><span class="punctuation-bracket">)</span><span class="punctuation-delimiter">.</span><span class="function-call">sum</span><span class="punctuation-bracket">(</span><span class="punctuation-bracket">)</span>
</div><div class="line" data-line="3"><span class="punctuation-bracket">&rbrace;</span>
</div><div class="line" data-line="4">
</div><div class="line" data-line="5"><span class="keyword-function">fn</span> <span class="function">sum_of_cubes_rhs</span><span class="punctuation-bracket">(</span><span class="variable-parameter">n</span><span class="punctuation-delimiter">:</span> <span class="type-builtin">usize</span><span class="punctuation-bracket">)</span> <span class="punctuation-delimiter">-&gt;</span> <span class="type-builtin">usize</span> <span class="punctuation-bracket">&lbrace;</span>
</div><div class="line" data-line="6">    <span class="keyword">let</span> <span class="variable">s</span> <span class="operator">=</span> <span class="variable">n</span> <span class="operator">*</span> <span class="punctuation-bracket">(</span><span class="variable">n</span> <span class="operator">+</span> <span class="number">1</span><span class="punctuation-bracket">)</span> <span class="operator">/</span> <span class="number">2</span><span class="punctuation-delimiter">;</span>
</div><div class="line" data-line="7">    <span class="variable">s</span> <span class="operator">*</span> <span class="variable">s</span>
</div><div class="line" data-line="8"><span class="punctuation-bracket">&rbrace;</span>
</div></code></pre>
```
_Generated HTML with style classes for the above code_

The language is identified through the language identifier of the code block, 
which is provided next to the opening tag of the codeblock. 
For example - ```` ```rust````.
If no language identifier is provided, the language is treated as plain text.

### Syntax for code sections

Similarly, there are two types of code displays,

####  Inline display - `fibonacci(5)`

Written as

```
`fibonacci(5)`
```

#### Block display

```rust
// rust
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
```

written as

````
```rust
// rust
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
```
````

---
