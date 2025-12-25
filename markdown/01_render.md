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

## Rendering math

$$
\sum_{i=1}^{n} i^3 = \left( \frac{n(n+1)}{2} \right) ^2
$$

### LaTeX

LaTeX is a high-quality typesetting system; it includes features designed for the production of technical and scientific documentation. LaTeX is the de facto standard for the communication and publication of scientific documents. LaTeX is available as free software. [[The Latex Project]](https://www.latex-project.org/)

LaTeX representation of the above equation:
```latex
\sum_{i=1}^{n} i^3 = \left( \frac{n(n+1)}{2} \right) ^2
```

However, web browsers and standard HTML cannot directly interpret LaTeX syntax. Therefore, the blog converts LaTeX into MathML Core.

### MathML

Mathematical Markup Language (MathML) is an XML-based language for describing mathematical notation.
MathML Core is a subset with increased implementation details based on rules from LaTeX and the Open Font Format. It is tailored for browsers and designed specifically to work well with other web standards including HTML, CSS, DOM, JavaScript.
[[MDN Web Docs]](https://developer.mozilla.org/en-US/docs/Web/MathML)

MathML representation of the above equation:
```html
<math display="block"><munderover><mo movablelimits="false">∑</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mrow><mi>n</mi></mrow></munderover><msup><mi>i</mi><mn>3</mn></msup><mo>=</mo><msup><mrow><mo stretchy="true">(</mo><mfrac><mrow><mi>n</mi><mo symmetric="false" stretchy="false">(</mo><mi>n</mi><mo>+</mo><mn>1</mn><mo symmetric="false" stretchy="false">)</mo></mrow><mrow><mn>2</mn></mrow></mfrac><mo stretchy="true">)</mo></mrow><mn>2</mn></msup></math>
```

### Pulldown-latex

To convert from LaTeX to MathML, [`pulldown-cmark`](https://crates.io/crates/pulldown-cmark) events are used to identify math sections. When the `pulldown_cmark::Parser` parses the markdown file, it detects the tags for both inline (`$`) and block (`$$`) math expressions. Then, the LaTeX inside these sections is converted to MathML using the [`pulldown-latex`](https://crates.io/crates/pulldown-latex) crate. The resultant MathML is streamed and the math is rendered.

---

## Highlighting code

```rust
fn sum_of_cubes_lhs(n: usize) -> usize {
    (2..=n).map(|i| i * i * i).sum()
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

When `pulldown-cmark` identifies a code block,
autumnus generates HTML with different CSS classes for different code elements
like keywords, variables and constants. This enables for syntax highlighting
through CSS. The `autumnus` repo provides several [CSS files](https://github.com/leandrocp/autumnus/tree/main/css), 

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

---

## Relevant code

The `pulldown_cmark::Parser` is a Markdown parser that returns an iterator of events rather than building a complete AST upfront. Events represent Markdown elements like

- `Event::Start(Tag::CodeBlock(...))` - start of a code block
- `Event::Text(...)` - text content
- `Event::End(TagEnd::CodeBlock)` - end of a code block
- `Event::DisplayMath(...)` - math sections

The `CustomIterator` wraps this event stream and transforms events lazily during iteration.

```rust
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, MetadataBlockKind, Options, Parser, Tag, TagEnd,
};
use pulldown_latex::{Storage, config::DisplayMode};

pub(crate) struct CustomIterator<'a, I: Iterator<Item = Event<'a>>> {
    inner: I,
    storage: Storage,
    code: Option<CowStr<'a>>,
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

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for CustomIterator<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                self.code = Some(lang);
                Some(Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)))
            }

            Event::Text(code) if let Some(lang) = self.code.as_mut() => {
                let highlighted = highlight_code(&code, lang).ok()?;
                Some(Event::Html(CowStr::from(highlighted)))
            }

            event @ Event::End(TagEnd::CodeBlock) if self.code.is_some() => {
                self.code = None;
                Some(event)
            }

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

Code highlighting with `autumnus`
```rust
use autumnus::{HtmlLinkedBuilder, formatter::Formatter, languages::Language};

pub(crate) fn highlight_code(code: &str, syntax_tag: &str) -> Result<String> {
    let formatter = HtmlLinkedBuilder::new()
        .source(code)
        .lang(match syntax_tag {
            "rust" => Language::Rust,
            "html" => Language::HTML,
            "latex" => Language::LaTeX,
            _ => Language::PlainText,
        })
        .pre_class(None)
        .build()?;

    let mut output = Vec::new();
    formatter.format(&mut output)?;
    let html = String::from_utf8(output)?;

    Ok(html)
}
```
