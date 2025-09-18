---
title: Rendering math and highlighting code
subtitle: How the blog renders math and highlights syntax, using only static HTML and CSS.
tags:
    - SSG
    - Blog
    - Rust
    - LaTeX
---

# Rendering math and highlighting code

This section explains how the blog renders math and highlights syntax, using only static HTML and CSS.
The code for this is located in the [`syntex`](https://github.com/shettysach/shettysach.github.io/blob/main/src/syntex.rs)
module, a portmanteau of syntax and TeX.

---

## Rendering math { #math }

$$
\sum_{i=1}^{n} i^3 = \left( \frac{n(n+1)}{2} \right) ^2
$$

### LaTeX

LaTeX is a high-quality typesetting system; it includes features designed for the production of technical and scientific documentation. LaTeX is the de facto standard for the communication and publication of scientific documents. LaTeX is available as free software. [[The Latex Project]](https://www.latex-project.org/)

```
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

To convert from LaTeX to MathML, [`pulldown_cmark`](https://crates.io/crates/pulldown-cmark) events are used to identify math and code sections. When `pulldown-cmark` parses the markdown file, it detects the tags for both inline and block math expressions. Then, the LaTeX inside these sections, is converted to MathML using the [`pulldown_latex`](https://crates.io/crates/pulldown-latex) crate. The resultant MathML is added to the output events vector and the math is rendered. 

Sum of cubes of the first $n$ natural numbers $\mathbb{N}$

$$
\sum_{i=1}^{n} i^3 = \left( \frac{n(n+1)}{2} \right) ^2
$$

### Syntax for math sections

There are two types of math displays,

#### Inline display

$F(n)$, written as

  ```
  $F(n)$
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
    
  ```tex
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
    (0..=n).map(|i| i * i * i).sum()
}

fn sum_of_cubes_rhs(n: usize) -> usize {
    let s = n * (n + 1) / 2;
    s * s
}
```

For syntax highlighting, the blog uses the [`syntect`](https://crates.io/crates/pulldown-cmark) crate,
which is also used in various other programs such as [`bat`](https://github.com/sharkdp/bat), the alternative
to `cat`.

When `pulldown-cmark` identifies a codeblock,
`syntect` generates HTML with CSS classes for different code elements
like keywords, variables and constants. This enables for syntax highlighting
through CSS classes, which makes it easy to have colours assigned to different elements.

`syntect` also generates themes from `.tmTheme` files. This blog uses [`Enki-Tokyo-Night`](https://raw.githubusercontent.com/enkia/enki-theme/refs/heads/master/scheme/Enki-Tokyo-Night.tmTheme) from the [Enki Theme by enkia](https://github.com/enkia/enki-theme).

```html
<pre><code><span class="source rust"><span class="meta function rust"><span class="meta function rust"><span class="storage type function rust">fn</span> </span><span class="entity name function rust">sum_of_cubes_lhs</span></span><span class="meta function rust"><span class="meta function parameters rust"><span class="punctuation section parameters begin rust">(</span><span class="variable parameter rust">n</span><span class="punctuation separator rust">:</span> <span class="storage type rust">usize</span></span><span class="meta function rust"><span class="meta function parameters rust"><span class="punctuation section parameters end rust">)</span></span></span></span><span class="meta function rust"> <span class="meta function return-type rust"><span class="punctuation separator rust">-&gt;</span> <span class="storage type rust">usize</span></span> </span><span class="meta function rust"><span class="meta block rust"><span class="punctuation section block begin rust">{</span>
    <span class="meta group rust"><span class="punctuation section group begin rust">(</span><span class="constant numeric integer decimal rust">0</span><span class="keyword operator rust">..</span><span class="keyword operator rust">=</span>n</span><span class="meta group rust"><span class="punctuation section group end rust">)</span></span>.<span class="support function rust">map</span><span class="meta group rust"><span class="punctuation section group begin rust">(</span><span class="meta function closure rust"><span class="meta function parameters rust"><span class="punctuation section parameters begin rust">|</span></span></span><span class="meta function closure rust"><span class="meta function parameters rust"><span class="variable parameter rust">i</span><span class="punctuation section parameters end rust">|</span></span> </span><span class="meta function closure rust">i <span class="keyword operator rust">*</span> i <span class="keyword operator rust">*</span> i</span></span><span class="meta group rust"><span class="punctuation section group end rust">)</span></span>.<span class="support function rust">sum</span><span class="meta group rust"><span class="punctuation section group begin rust">(</span></span><span class="meta group rust"><span class="punctuation section group end rust">)</span></span>
</span><span class="meta block rust"><span class="punctuation section block end rust">}</span></span></span>

<span class="meta function rust"><span class="meta function rust"><span class="storage type function rust">fn</span> </span><span class="entity name function rust">sum_of_cubes_rhs</span></span><span class="meta function rust"><span class="meta function parameters rust"><span class="punctuation section parameters begin rust">(</span><span class="variable parameter rust">n</span><span class="punctuation separator rust">:</span> <span class="storage type rust">usize</span></span><span class="meta function rust"><span class="meta function parameters rust"><span class="punctuation section parameters end rust">)</span></span></span></span><span class="meta function rust"> <span class="meta function return-type rust"><span class="punctuation separator rust">-&gt;</span> <span class="storage type rust">usize</span></span> </span><span class="meta function rust"><span class="meta block rust"><span class="punctuation section block begin rust">{</span>
    <span class="storage type rust">let</span> s <span class="keyword operator rust">=</span> n <span class="keyword operator rust">*</span> <span class="meta group rust"><span class="punctuation section group begin rust">(</span>n <span class="keyword operator rust">+</span> <span class="constant numeric integer decimal rust">1</span></span><span class="meta group rust"><span class="punctuation section group end rust">)</span></span> <span class="keyword operator rust">/</span> <span class="constant numeric integer decimal rust">2</span><span class="punctuation terminator rust">;</span>
    s <span class="keyword operator rust">*</span> s
</span><span class="meta block rust"><span class="punctuation section block end rust">}</span></span></span>
</span></code></pre>
```
_Generated HTML with style classes for the above code_

The language is identified through the language identifier of the code block, 
which is provided next to the opening tag of the codeblock. 
For example - ```` ```rust````.
If no language identifier is provided, the language is treated as plain text.

### Syntax for code sections

Similarly, there are two types of code displays,

####  Inline display

  `fibonacci(5)`, written as

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
