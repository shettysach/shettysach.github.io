use anyhow::Result;
use autumnus::{HtmlLinkedBuilder, formatter::Formatter, languages::Language};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, MetadataBlockKind, Options, Parser, Tag, TagEnd,
};
use pulldown_latex::{RenderConfig, Storage, config::DisplayMode, mathml::push_mathml};
use yaml_rust2::{Yaml, YamlLoader};

pub(crate) const OPTIONS: Options = Options::empty()
    .union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS)
    .union(Options::ENABLE_MATH)
    .union(Options::ENABLE_HEADING_ATTRIBUTES);

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
        let event = self.inner.next()?;

        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                self.code = Some(lang);
                self.next()
            }

            Event::Text(code) if let Some(lang) = self.code.as_mut() => {
                let highlighted = highlight_code(&code, lang).ok()?;
                Some(Event::Html(CowStr::from(highlighted)))
            }

            Event::End(TagEnd::CodeBlock) if self.code.is_some() => {
                self.code = None;
                self.next()
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

            _ => Some(event),
        }
    }
}

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

pub(crate) fn process_metadata(mut parser: Parser) -> Option<Yaml> {
    if let Some(Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle))) = parser.next()
        && let Some(Event::Text(yaml)) = parser.next()
        && let Some(Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle))) = parser.next()
    {
        YamlLoader::load_from_str(&yaml).ok()?.into_iter().next()
    } else {
        None
    }
}
