use anyhow::Result;
use lumis::{HtmlLinkedBuilder, languages::Language};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, MetadataBlockKind, Options, Parser, Tag, TagEnd,
};
use pulldown_latex::{RenderConfig, Storage, config::DisplayMode, mathml::push_mathml};
use yaml_rust2::{Yaml, YamlLoader};

pub(crate) const OPTIONS: Options = Options::empty()
    .union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS)
    .union(Options::ENABLE_MATH)
    .union(Options::ENABLE_HEADING_ATTRIBUTES)
    .union(Options::ENABLE_FOOTNOTES);

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

            // -- Math
            Event::DisplayMath(latex) => Some(Event::Html(CowStr::from(
                latex_to_mathml(&latex, &mut self.storage, DisplayMode::Block).ok()?,
            ))),

            Event::InlineMath(latex) => Some(Event::InlineHtml(CowStr::from(
                latex_to_mathml(&latex, &mut self.storage, DisplayMode::Inline).ok()?,
            ))),

            event => Some(event),
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

pub(crate) fn highlight_code(code: &str, tag: &str) -> Result<String> {
    let formatter = HtmlLinkedBuilder::new()
        .lang(match tag {
            "rust" | "rs" => Language::Rust,
            "python" | "py" => Language::Python,
            "latex" | "tex" => Language::LaTeX,
            "html" => Language::HTML,
            _ => Language::PlainText,
        })
        .pre_class(None)
        .build()?;

    Ok(lumis::highlight(code, formatter))
}

pub(crate) fn process_metadata(parser: &mut Parser) -> Option<Yaml> {
    if let Some(Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle))) = parser.next()
        && let Some(Event::Text(yaml)) = parser.next()
        && let Some(Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle))) = parser.next()
    {
        YamlLoader::load_from_str(&yaml).ok()?.into_iter().next()
    } else {
        None
    }
}
