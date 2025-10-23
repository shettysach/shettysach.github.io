use crate::types::Frontmatter;
use anyhow::Result;
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, MetadataBlockKind, Options, Parser, Tag, TagEnd,
};
use pulldown_latex::{RenderConfig, Storage, config::DisplayMode, mathml::push_mathml};
use std::sync::OnceLock;
use syntect::{
    html::{ClassStyle, ClassedHTMLGenerator},
    parsing::SyntaxSet,
};
use yaml_rust2::{Yaml, YamlLoader};

pub(crate) const OPTIONS: Options = Options::empty()
    .union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS)
    .union(Options::ENABLE_MATH)
    .union(Options::ENABLE_HEADING_ATTRIBUTES);

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

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
    let syntax_set = SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines);
    let syntax = syntax_token
        .and_then(|t| syntax_set.find_syntax_by_token(t))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let mut class_gen =
        ClassedHTMLGenerator::new_with_class_style(syntax, syntax_set, ClassStyle::Spaced);

    for line in code.split_inclusive('\n') {
        class_gen.parse_html_for_line_which_includes_newline(line)?
    }

    Ok(format!("<pre><code>{}</code></pre>", class_gen.finalize()))
}

pub(crate) fn process_metadata(mut parser: Parser) -> Option<(Frontmatter, bool)> {
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

fn parse_metadata(docs: Vec<Yaml>) -> Option<(Frontmatter, bool)> {
    let doc = docs.first()?;
    let title = doc["title"].as_str()?.to_string();
    let subtitle = doc["subtitle"].as_str().map(str::to_string);
    let tags = doc["tags"]
        .as_vec()
        .and_then(|vec| vec.iter().map(|v| v.as_str().map(String::from)).collect());
    let create_toc = doc["toc"].as_bool().is_some_and(|t| t);

    Some((
        Frontmatter {
            title,
            subtitle,
            tags,
        },
        create_toc,
    ))
}

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

pub(crate) fn table_bullet(level: HeadingLevel, heading: &str, anchor_id: &str) -> String {
    let indent = "  ".repeat(match level {
        HeadingLevel::H1 => 0,
        HeadingLevel::H2 => 1,
        HeadingLevel::H3 => 2,
        HeadingLevel::H4 => 3,
        HeadingLevel::H5 => 4,
        HeadingLevel::H6 => 5,
    });
    format!("{indent}- [{heading}](#{anchor_id})\n")
}
