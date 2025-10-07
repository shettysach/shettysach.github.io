use anyhow::{Error, Result};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, MetadataBlockKind, Options, Tag, TagEnd,
};
use pulldown_latex::{RenderConfig, Storage, config::DisplayMode, mathml::push_mathml};
use slug::slugify;
use syntect::{
    html::{ClassStyle, ClassedHTMLGenerator},
    parsing::{SyntaxReference, SyntaxSet},
};
use yaml_rust2::{Yaml, YamlLoader};

pub(crate) const OPTIONS: Options = Options::empty()
    .union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS)
    .union(Options::ENABLE_MATH)
    .union(Options::ENABLE_HEADING_ATTRIBUTES);

pub(crate) struct Article<'a> {
    pub(crate) metadata: Metadata,
    pub(crate) events: Vec<Event<'a>>,
    pub(crate) toc: Option<String>,
}

pub(crate) struct Metadata {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) tags: Option<Vec<String>>,
}

pub(crate) trait Syntex<'a> {
    fn process(self, syntax_set: &SyntaxSet) -> Result<Article<'a>>;
}

impl<'a> Syntex<'a> for pulldown_cmark::Parser<'a> {
    fn process(self: pulldown_cmark::Parser<'a>, syntax_set: &SyntaxSet) -> Result<Article<'a>> {
        let plain_text = syntax_set.find_syntax_plain_text();

        let mut syntax = plain_text;
        let mut storage = Storage::new();

        let mut capture = false;
        let mut toc: Option<String> = None;

        let mut captive = String::new();

        let mut metadata_init = None;
        let mut events = Vec::new();

        for event in self {
            match event {
                Event::Text(t) if capture => captive.push_str(&t),

                Event::Text(t) => events.push(Event::Html(t)),

                Event::Start(Tag::Heading { .. }) if toc.is_some() => capture = true,

                Event::End(TagEnd::Heading(level)) => {
                    if let Some(table) = toc.as_mut() {
                        let anchor_str = slugify(&captive);
                        capture = false;

                        table.push_str(&table_bullet(level, &captive, &anchor_str));

                        events.push(Event::Start(Tag::Heading {
                            level,
                            id: Some(CowStr::from(anchor_str)),
                            classes: Vec::new(),
                            attrs: Vec::new(),
                        }));

                        let text = std::mem::take(&mut captive);
                        events.push(Event::Html(CowStr::from(text)));
                    }

                    events.push(event);
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

                    (metadata_init, toc) = parse_metadata(docs)
                        .map(|(init, has_toc)| (Some(init), has_toc.then(String::new)))
                        .unwrap_or((None, None));

                    captive.clear();
                    capture = false;
                }

                _ => events.push(event),
            }
        }

        metadata_init
            .map(|metadata| Article {
                metadata,
                events,
                toc,
            })
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

fn parse_metadata(docs: Vec<Yaml>) -> Option<(Metadata, bool)> {
    let doc = docs.first()?;
    let title = doc["title"].as_str()?.to_string();
    let subtitle = doc["subtitle"].as_str().map(str::to_string);
    let tags = doc["tags"]
        .as_vec()
        .and_then(|vec| vec.iter().map(|v| v.as_str().map(String::from)).collect());
    let create_toc = doc["toc"].as_bool().is_some_and(|t| t);

    Some((
        Metadata {
            title,
            subtitle,
            tags,
        },
        create_toc,
    ))
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
    parse_metadata(docs)
        .ok_or_else(|| Error::msg("Failed to parse metadata"))
        .map(|x| x.0)
}

pub(crate) fn table_bullet(level: HeadingLevel, heading: &str, anchor: &str) -> String {
    let indent = "  ".repeat(match level {
        HeadingLevel::H1 => 0,
        HeadingLevel::H2 => 1,
        HeadingLevel::H3 => 2,
        HeadingLevel::H4 => 3,
        HeadingLevel::H5 => 4,
        HeadingLevel::H6 => 5,
    });
    format!("{indent}- [{heading}](#{anchor})\n")
}
