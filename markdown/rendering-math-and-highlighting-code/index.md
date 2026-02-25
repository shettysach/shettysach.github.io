---
title: Rendering math and highlighting code
subtitle: How the blog renders math and highlights syntax, using static HTML and CSS
date: 2025-06-05
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

## Pull parser

The blog uses `pulldown_cmark::Parser` from the [`pulldown-cmark`](https://crates.io/crates/pulldown-cmark) crate, which is a pull parser for [CommonMark](https://commonmark.org/).
A pull parser is a streaming parser where the caller repeatedly invokes `next()` to pull the next event from the input, 
rather than building a complete AST upfront. 
This design works well with Rust's lazy iterators and is efficient for large files.

The `CustomIterator` wraps this event stream and transforms events lazily during iteration.

```rust
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, MetadataBlockKind, Options, Parser, Tag, TagEnd,
};
use pulldown_latex::{Storage, config::DisplayMode};

pub(crate) struct CustomIterator<'a, I: Iterator<Item = Event<'a>>> {
    inner: I,                 // The pulldown-cmark parser
    storage: Storage,         // Used for math rendering
    code: Option<CowStr<'a>>, // Used for code highlighting
}

impl<'a, I: Iterator<Item = Event<'a>>> CustomIterator<'a, I> {
    pub(crate) fn new(inner: I) -> Self {
        Self {
            inner,
            storage: Storage::new(),
            code: None,
        }
    }
}
```

---

## Rendering math

$$
\sum_{i=1}^{n} i^3 = \left( \frac{n(n+1)}{2} \right) ^2
$$

### LaTeX

LaTeX is a high-quality typesetting system; it includes features designed for the production of technical and scientific documentation. LaTeX is the de facto standard for the communication and publication of scientific documents. LaTeX is available as free software. [^1]

LaTeX representation of the above equation:
```latex
\sum_{i=1}^{n} i^3 = \left( \frac{n(n+1)}{2} \right) ^2
```

However, web browsers and standard HTML cannot directly interpret LaTeX syntax. Therefore, the blog converts LaTeX into MathML Core.

### MathML

Mathematical Markup Language (MathML) is an XML-based language for describing mathematical notation.
MathML Core is a subset with increased implementation details based on rules from LaTeX and the Open Font Format. It is tailored for browsers and designed specifically to work well with other web standards including HTML, CSS, DOM, JavaScript. [^2]

MathML representation of the above equation:
```html
<math display="block"><munderover><mo movablelimits="false">∑</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mrow><mi>n</mi></mrow></munderover><msup><mi>i</mi><mn>3</mn></msup><mo>=</mo><msup><mrow><mo stretchy="true">(</mo><mfrac><mrow><mi>n</mi><mo symmetric="false" stretchy="false">(</mo><mi>n</mi><mo>+</mo><mn>1</mn><mo symmetric="false" stretchy="false">)</mo></mrow><mrow><mn>2</mn></mrow></mfrac><mo stretchy="true">)</mo></mrow><mn>2</mn></msup></math>
```

### Pulldown-latex

To convert from LaTeX to MathML, pulldown-cmark events are used to identify math sections. When the `pulldown_cmark::Parser` parses the markdown file, it detects the tags for both inline (`$`) and block (`$$`) math expressions. Then, the LaTeX inside these sections is converted to MathML using the [`pulldown-latex`](https://crates.io/crates/pulldown-latex) crate. The resultant MathML is streamed and the math is rendered.

### Relevant code

These are the relevant events we deal with -

- `Event::DisplayMath(...)` - block math sections, enclosed in `$$ ... $$`
- `Event::InlineMath(...)` - inline math sections, enclosed in `$ ... $`

```rust
impl<'a, I: Iterator<Item = Event<'a>>> Iterator for CustomIterator<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            // ...

            Event::DisplayMath(latex) => {
                let mathml = latex_to_mathml(&latex, &mut self.storage, DisplayMode::Block).ok()?;
                Some(Event::Html(CowStr::from(mathml)))
            }

            Event::InlineMath(latex) => {
                let mathml =
                    latex_to_mathml(&latex, &mut self.storage, DisplayMode::Inline).ok()?;
                Some(Event::InlineHtml(CowStr::from(mathml)))
            }

            event => Some(event),
        }
    }
}
```

Math rendering with `pulldown_latex`
```rust
use pulldown_latex::{RenderConfig, Storage, config::DisplayMode, mathml::push_mathml};

pub(crate) fn latex_to_mathml(
    latex: &str,
    storage: &mut Storage,
    display_mode: DisplayMode,
) -> Result<String> {
    let mut mathml = String::new();
    let parser = pulldown_latex::Parser::new(latex, storage);

    let config = RenderConfig {
        display_mode,
        ..Default::default()
    };

    push_mathml(&mut mathml, parser, config)?;
    storage.reset();
    Ok(mathml)
}
```

---

## Highlighting code

```rust
fn sum_of_cubes_lhs(n: usize) -> usize {
    (1..=n).map(|i| i * i * i).sum()
}

fn sum_of_cubes_rhs(n: usize) -> usize {
    let s = n * (n + 1) / 2;
    s * s
}
```

### Autumnus and Tree-sitter

For syntax highlighting, the blog uses the [`autumnus`](https://crates.io/crates/autumnus) crate,
which uses [`tree-sitter`](https://github.com/tree-sitter/tree-sitter) under the hood.
Tree-sitter powers the highlighting in editors such as
[Neovim](https://neovim.io/doc/user/treesitter.html) and [Zed](https://zed.dev/blog/syntax-aware-editing).

When `pulldown-cmark` identifies a code block with a language tag,
`autumnus` generates HTML with different CSS classes for different code elements
like keywords, variables, and constants for the language specified with the tag. 
This enables for syntax highlighting through CSS styles and the `autumnus` repo provides several [CSS files for various themes](https://github.com/leandrocp/autumnus/tree/main/css) such as Tokyo Night and Catppuccin. 

Generated HTML with style classes for the above code:

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

### Relevant code

These are the relevant events we deal with -

- `Event::Start(Tag::CodeBlock(Fenced(lang)))` - start of a code block, `lang` is the language tag
- `Event::Text(...)` - text content within the code block
- `Event::End(TagEnd::CodeBlock)` - end of a code block

```rust
impl<'a, I: Iterator<Item = Event<'a>>> Iterator for CustomIterator<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            // -- Code --
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                self.code = Some(lang);
                self.next()
            }

            Event::Text(code) if let Some(lang) = self.code.as_mut() => {
                Some(Event::Html(CowStr::from(highlight_code(&code, lang).ok()?)))
            }

            Event::End(TagEnd::CodeBlock) if self.code.is_some() => {
                self.code = None;
                self.next()
            }

            // ..

            event => Some(event),
        }
    }
}
```

Code highlighting with `autumnus`

```rust
use autumnus::{HtmlLinkedBuilder, formatter::Formatter, languages::Language};

pub(crate) fn highlight_code(code: &str, tag: &str) -> Result<String> {
    let formatter = HtmlLinkedBuilder::new()
        .source(code)
        .lang(match tag {
            "rust" | "rs" => Language::Rust,
            "python" | "py" => Language::Python,
            "latex" | "tex" => Language::LaTeX,
            "html" => Language::HTML,
            _ => Language::PlainText,
        })
        .pre_class(None)
        .build()?;

    let mut output = Vec::new();
    formatter.highlights(&mut output)?;
    let html = String::from_utf8(output)?;

    Ok(html)
}
```

---

[^1]: From [The LaTeX Project](https://www.latex-project.org/).

[^2]: From the [MDN Web Docs](https://developer.mozilla.org/en-US/docs/Web/MathML).
