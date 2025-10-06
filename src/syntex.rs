use crate::generate::{Article, Metadata};
use anyhow::{Error, Result};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, MetadataBlockKind, Options, Tag, TagEnd};
use pulldown_latex::{RenderConfig, Storage, config::DisplayMode, mathml::push_mathml};
use syntect::{
    html::{ClassStyle, ClassedHTMLGenerator},
    parsing::{SyntaxReference, SyntaxSet},
};
use yaml_rust2::{Yaml, YamlLoader};

pub(crate) const OPTIONS: Options = Options::empty()
    .union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS)
    .union(Options::ENABLE_MATH)
    .union(Options::ENABLE_HEADING_ATTRIBUTES);

pub(crate) trait Syntex<'a> {
    fn process(self, syntax_set: &SyntaxSet) -> Result<Article<'a>>;
}

impl<'a> Syntex<'a> for pulldown_cmark::Parser<'a> {
    fn process(self: pulldown_cmark::Parser<'a>, syntax_set: &SyntaxSet) -> Result<Article<'a>> {
        let plain_text = syntax_set.find_syntax_plain_text();

        let mut syntax = plain_text;
        let mut storage = Storage::new();

        let mut anchored = false;
        let mut capture = false;
        let mut captive = String::new();

        let mut metadata_init = None;
        let mut events = Vec::with_capacity(1000); // Rough estimate to reduce reallocations

        for event in self {
            match event {
                Event::Text(t) if capture => captive.push_str(&t),

                Event::Text(t) => events.push(Event::Html(t)),

                // WARN: Apparently invalid html5, but works due to browser leniency
                // Replace <a><h> with <h><a>
                Event::Start(Tag::Heading { ref id, .. }) => {
                    if let Some(id) = id {
                        anchored = true;
                        let anchor = Event::Html(CowStr::from(format!("<a href=\"#{id}\">")));
                        events.push(anchor);
                    }
                    events.push(event);
                }

                Event::End(TagEnd::Heading(_)) if anchored => {
                    let anchor_end = Event::Html(CowStr::Borrowed("</a>"));
                    events.push(event);
                    events.push(anchor_end);

                    anchored = false;
                }

                Event::Start(Tag::CodeBlock(kind)) => {
                    capture = true;
                    syntax = match kind {
                        CodeBlockKind::Fenced(lang) => {
                            syntax_set.find_syntax_by_token(&lang).unwrap_or(plain_text)
                        }
                        CodeBlockKind::Indented => plain_text,
                    };
                }

                Event::End(TagEnd::CodeBlock) => {
                    let highlighted = highlight_code(&captive, syntax, syntax_set)?;
                    let event = Event::Html(CowStr::from(highlighted));
                    events.push(event);

                    capture = false;
                    captive.clear();
                }

                Event::DisplayMath(latex) => {
                    let mathml = latex_to_mathml(&latex, &mut storage, DisplayMode::Block)?;
                    let event = Event::Html(CowStr::from(mathml));
                    events.push(event);
                }

                Event::InlineMath(latex) => {
                    let mathml = latex_to_mathml(&latex, &mut storage, DisplayMode::Inline)?;
                    let event = Event::InlineHtml(CowStr::from(mathml));
                    events.push(event);
                }

                Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => capture = true,

                Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                    let docs = YamlLoader::load_from_str(&captive)?;
                    metadata_init = parse_metadata(docs);

                    captive.clear();
                    capture = false;
                }

                _ => events.push(event),
            }
        }

        metadata_init
            .map(|metadata| Article { metadata, events })
            .ok_or_else(|| Error::msg("Metadata error"))
    }
}

fn latex_to_mathml(
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

fn highlight_code(code: &str, syntax: &SyntaxReference, syntax_set: &SyntaxSet) -> Result<String> {
    let mut class_gen =
        ClassedHTMLGenerator::new_with_class_style(syntax, syntax_set, ClassStyle::Spaced);

    for line in code.split_inclusive('\n') {
        class_gen.parse_html_for_line_which_includes_newline(line)?
    }

    Ok(format!("<pre><code>{}</code></pre>", class_gen.finalize()))
}

fn parse_metadata(docs: Vec<Yaml>) -> Option<Metadata> {
    let doc = docs.first()?;
    let title = doc["title"].as_str()?.to_string();
    let subtitle = doc["subtitle"].as_str().map(String::from);
    let tags = doc["tags"]
        .as_vec()
        .and_then(|vec| vec.iter().map(|v| v.as_str().map(String::from)).collect());

    Some(Metadata {
        title,
        subtitle,
        tags,
    })
}

pub(crate) fn extract_metadata(markdown: &str) -> Result<Metadata> {
    let start = markdown
        .find("---\n")
        .ok_or_else(|| Error::msg("No frontmatter found"))?;
    let rest = &markdown[start + 4..];
    let end = rest
        .find("\n---\n")
        .ok_or_else(|| Error::msg("Invalid frontmatter"))?;
    let yaml_str = &rest[..end];
    let docs = YamlLoader::load_from_str(yaml_str)?;
    parse_metadata(docs).ok_or_else(|| Error::msg("Failed to parse metadata"))
}
