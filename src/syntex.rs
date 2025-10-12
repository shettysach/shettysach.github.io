use crate::utils::Slugger;
use anyhow::{Error, Result};
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

pub(crate) struct Article<'a> {
    pub(crate) metadata: Metadata,
    pub(crate) events: Vec<Event<'a>>,
    pub(crate) toc: Option<String>,
}

#[derive(Clone)]
pub(crate) struct Metadata {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) tags: Option<Vec<String>>,
}

pub(crate) trait Syntex<'a> {
    fn process(self) -> Result<Article<'a>>;
}

impl<'a> Syntex<'a> for Parser<'a> {
    fn process(self: Parser<'a>) -> Result<Article<'a>> {
        let mut syntax_token: Option<CowStr> = None;
        let mut storage = Storage::new();

        let mut toc: Option<String> = None;

        let mut captive_string: Option<String> = None;
        let mut captive_heading = None;
        let mut captive_events: Vec<Event<'a>> = Vec::new();

        let mut metadata_init = None;

        let bound = 20;
        let mut events = Vec::with_capacity(bound);
        let mut slugger = Slugger::with_capacity(bound / 5);

        for event in self {
            match event {
                Event::Text(t) => {
                    if let Some(s) = captive_string.as_mut() {
                        s.push_str(&t);
                        if captive_heading.is_some() {
                            captive_events.push(Event::Text(t));
                        }
                    } else {
                        events.push(Event::Text(t));
                    }
                }

                Event::Start(Tag::Heading {
                    level,
                    id,
                    classes,
                    attrs,
                }) if toc.is_some() => {
                    captive_heading = Some((level, id, classes, attrs));
                    captive_string = Some(String::new());
                }

                Event::End(TagEnd::Heading(_)) => {
                    if let Some(table) = toc.as_mut() {
                        let (level, id, classes, attrs) =
                            unsafe { captive_heading.take().unwrap_unchecked() };

                        if let Some(header_text) = captive_string.take() {
                            let id = id.unwrap_or_else(|| CowStr::from(slugger.slug(&header_text)));
                            table.push_str(&table_bullet(level, &header_text, &id));

                            events.push(Event::Start(Tag::Heading {
                                level,
                                id: Some(id),
                                classes,
                                attrs,
                            }));

                            events.append(&mut captive_events);
                            captive_events.clear();
                        }
                    }

                    events.push(event);
                }

                Event::Code(code) if captive_heading.is_some() => {
                    if let Some(s) = captive_string.as_mut() {
                        s.push_str(&code);
                    }
                    captive_events.push(Event::Code(code));
                }

                Event::Start(Tag::CodeBlock(kind)) => {
                    captive_string = Some(String::new());
                    syntax_token = match kind {
                        CodeBlockKind::Fenced(lang) => Some(lang),
                        CodeBlockKind::Indented => None,
                    };
                }

                Event::End(TagEnd::CodeBlock) => {
                    if let Some(code) = captive_string.take() {
                        let highlighted = highlight_code(&code, syntax_token.as_deref())?;
                        let event = Event::Html(CowStr::from(highlighted));
                        events.push(event);
                    }
                }

                Event::DisplayMath(latex) => {
                    let mathml = latex_to_mathml(&latex, &mut storage, DisplayMode::Block)?;
                    let event = Event::Html(CowStr::from(mathml));
                    events.push(event);
                }

                Event::InlineMath(latex) => {
                    let mathml = latex_to_mathml(&latex, &mut storage, DisplayMode::Inline)?;
                    let event = Event::InlineHtml(CowStr::from(mathml));

                    if captive_heading.is_some() {
                        if let Some(s) = captive_string.as_mut() {
                            s.push_str(&latex);
                        }
                        captive_events.push(event);
                    } else {
                        events.push(event);
                    }
                }

                Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                    captive_string = Some(String::new())
                }

                Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                    if let Some(yaml) = captive_string.take() {
                        let docs = YamlLoader::load_from_str(&yaml)?;

                        (metadata_init, toc) = parse_metadata(docs)
                            .map(|(init, has_toc)| (Some(init), has_toc.then(String::new)))
                            .unwrap_or((None, None));
                    }
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

fn highlight_code(code: &str, syntax_token: Option<&str>) -> Result<String> {
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

pub(crate) fn extract_metadata(markdown: &str) -> Option<Metadata> {
    let start = markdown.find("---\n")?;
    let rest = &markdown[start + 4..];
    let end = rest.find("\n---\n")?;
    let yaml_str = &rest[..end];
    let docs = YamlLoader::load_from_str(yaml_str).ok()?;
    parse_metadata(docs).map(|x| x.0)
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
