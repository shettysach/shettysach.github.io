use crate::types::{Frontmatter, Levels};
use anyhow::Result;
use autumnus::{HtmlLinkedBuilder, formatter::Formatter, languages::Language};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, MetadataBlockKind, Parser, Tag, TagEnd};
use pulldown_latex::{RenderConfig, Storage, config::DisplayMode, mathml::push_mathml};
use yaml_rust2::{Yaml, YamlLoader};

pub(crate) const OPTIONS: pulldown_cmark::Options = pulldown_cmark::Options::empty()
    .union(pulldown_cmark::Options::ENABLE_YAML_STYLE_METADATA_BLOCKS)
    .union(pulldown_cmark::Options::ENABLE_MATH)
    .union(pulldown_cmark::Options::ENABLE_HEADING_ATTRIBUTES);

pub(crate) struct CustomIterator<'a, I: Iterator<Item = Event<'a>>> {
    inner: I,
    syntax_token: Option<CowStr<'a>>,
    storage: Storage,
    captive_string: Option<String>,
}

impl<'a, I: Iterator<Item = Event<'a>>> CustomIterator<'a, I> {
    pub(crate) fn new(inner: I) -> Self {
        Self {
            inner,
            syntax_token: None,
            storage: Storage::new(),
            captive_string: None,
        }
    }
}

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for CustomIterator<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let event = self.inner.next()?;

        match event {
            Event::Text(ref t) => {
                if let Some(s) = self.captive_string.as_mut() {
                    s.push_str(t);
                    self.next()
                } else {
                    Some(event)
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                self.captive_string = Some(String::new());
                self.syntax_token = match kind {
                    CodeBlockKind::Fenced(lang) => Some(lang),
                    CodeBlockKind::Indented => None,
                };
                self.next()
            }

            Event::End(TagEnd::CodeBlock) => {
                if let Some(code) = self.captive_string.take() {
                    let highlighted = highlight_code(&code, self.syntax_token.as_deref()).ok()?;
                    return Some(Event::Html(CowStr::from(highlighted)));
                }
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

pub(crate) fn highlight_code(code: &str, syntax_token: Option<&str>) -> Result<String> {
    let formatter = HtmlLinkedBuilder::new()
        .source(code)
        .lang(match syntax_token {
            Some("rust") => Language::Rust,
            Some("html") => Language::HTML,
            Some("latex") => Language::LaTeX,
            _ => Language::PlainText,
        })
        .pre_class(None)
        .build()?;

    let mut output = Vec::new();
    formatter.format(&mut output)?;
    let html = String::from_utf8(output)?;

    Ok(html)
}

pub(crate) fn process_metadata(mut parser: Parser) -> Option<(Frontmatter, Option<Levels>)> {
    match parser.next() {
        Some(Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle))) => {}
        _ => return None,
    }

    let yaml = match parser.next() {
        Some(Event::Text(text)) => text,
        _ => return None,
    };

    match parser.next() {
        Some(Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle))) => {}
        _ => return None,
    }

    parse_metadata(YamlLoader::load_from_str(&yaml).ok()?)
}

fn parse_metadata(docs: Vec<Yaml>) -> Option<(Frontmatter, Option<Levels>)> {
    let doc = docs.first()?;
    let title = doc["title"].as_str()?.to_string();
    let subtitle = doc["subtitle"].as_str().map(str::to_string);
    let tags = doc["tags"]
        .as_vec()
        .and_then(|vec| vec.iter().map(|v| v.as_str().map(str::to_string)).collect());

    let levels = doc["hmin"]
        .as_i64()
        .zip(doc["hmax"].as_i64())
        .and_then(|(min, max)| {
            let min: u8 = min.try_into().ok()?;
            let max: u8 = max.try_into().ok()?;
            Levels::new(min, max)
        });

    Some((
        Frontmatter {
            title,
            subtitle,
            tags,
        },
        levels,
    ))
}
