use anyhow::{Context, Result};
use pulldown_cmark::{Event, Options, Parser, html};
use std::{collections::BTreeMap, fmt::Write, fs, path::Path};
use syntect::parsing::SyntaxSet;
use walkdir::WalkDir;

use crate::syntex::Syntex;

const HEADER: &str = include_str!("../layout/header.html");
const FOOTER: &str = include_str!("../layout/footer.html");
const OPTIONS: Options = Options::empty()
    .union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS)
    .union(Options::ENABLE_MATH)
    .union(Options::ENABLE_HEADING_ATTRIBUTES);

pub(crate) struct Article<'a> {
    pub(crate) metadata: Metadata,
    pub(crate) events: Vec<Event<'a>>,
}

pub(crate) struct Metadata {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) tags: Option<Vec<String>>,
}

impl Metadata {
    fn generate_header(&self) -> String {
        let title = format!("{} | Sachith Shetty", self.title);
        let description = self.subtitle.as_deref().unwrap_or("Blogpost");

        HEADER
            .replace("{{TITLE}}", &title)
            .replace("{{DESCRIPTION}}", description)
    }

    fn generate_tag(&self, tag: &str) -> String {
        format!("<a href=\"tags.html#{tag}\"><em>{tag}</em></a>")
    }

    fn render_label(&self, link: &Path) -> String {
        let mut label = format!(
            "<h2><a href=\"{}\">{}</a></h2>",
            link.to_string_lossy(),
            self.title
        );

        if let Some(ref subtitle) = self.subtitle {
            label.push_str(subtitle);
        }

        label
    }

    fn render_with_tags(&self, link: &Path) -> String {
        let mut label = self.render_label(link);

        if let Some(ref tags) = self.tags {
            if self.subtitle.is_some() {
                label.push_str("<br>");
            }

            for tag in &tags[..tags.len() - 1] {
                label.push_str(&self.generate_tag(tag));
                label.push_str(", ");
            }
            let tag = &tags[tags.len() - 1];
            label.push_str(&self.generate_tag(tag));
        }

        label
    }
}

fn render_markdown(src: &Path, dst: &Path, syntax_set: &SyntaxSet) -> Result<Metadata> {
    let markdown = fs::read_to_string(src)
        .with_context(|| format!("Failed to read markdown file: {}", src.display()))?;

    let Article { events, metadata } = Parser::new_ext(&markdown, OPTIONS)
        .process(syntax_set)
        .with_context(|| format!("Failed to process markdown: {}", src.display()))?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let mut page = String::with_capacity(HEADER.len() + markdown.len() + FOOTER.len());
    page.push_str(&metadata.generate_header());
    html::push_html(&mut page, events.into_iter());
    page.push_str(FOOTER);

    fs::write(dst, page)
        .with_context(|| format!("Failed to write HTML file: {}", dst.display()))?;

    Ok(metadata)
}

pub(crate) fn index_page(markdown_dir: &Path, html_dir: &Path) -> Result<()> {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let mut tags_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // Process index.md
    let index_md =
        fs::read_to_string(markdown_dir.join("index.md")).context("Failed to read index.md")?;

    let Article { metadata, events } = Parser::new_ext(&index_md, OPTIONS)
        .process(&syntax_set)
        .context("Failed to process index.md")?;

    let mut index_html = String::new();
    index_html.push_str(&metadata.generate_header());
    html::push_html(&mut index_html, events.into_iter());
    index_html.push_str("<ul>\n");

    for entry in WalkDir::new(markdown_dir)
        .max_depth(2)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
    {
        let src_path = entry.path();
        let rel_path = src_path.strip_prefix(markdown_dir)?;
        let dst_path = html_dir.join(rel_path);

        if src_path.extension().is_some_and(|ext| ext == "md") {
            if src_path.file_name().is_some_and(|name| name == "index.md") {
                continue;
            }

            let html_dst = dst_path.with_extension("html");
            let metadata = render_markdown(src_path, &html_dst, &syntax_set)?;
            let link = rel_path.with_extension("html");

            // Add to index HTML
            let label_with_tags = metadata.render_with_tags(&link);
            writeln!(index_html, "<li>{}</li>", label_with_tags)?;

            // Populate tags map
            if let Some(ref tags) = metadata.tags {
                for tag in tags {
                    tags_map
                        .entry(tag.clone())
                        .or_default()
                        .push(metadata.render_label(&link));
                }
            }
        } else if src_path.is_dir() {
            fs::create_dir_all(&dst_path).with_context(|| "Failed to create _site dir")?
        } else {
            fs::copy(src_path, dst_path)
                .with_context(|| format!("Failed to copy file: {}", src_path.display()))
                .map(|_| {})?
        }
    }

    index_html.push_str("</ul>\n");
    index_html.push_str(FOOTER);

    fs::write(html_dir.join("index.html"), index_html)
        .with_context(|| "Failed to write index.html")?;

    tags_page(tags_map, &html_dir.join("tags.html"))?;

    Ok(())
}

fn tags_page(tags_map: BTreeMap<String, Vec<String>>, tags_path: &Path) -> Result<()> {
    let estimated_size = tags_map.len() * 300 + HEADER.len() + FOOTER.len();
    let mut article_html = String::with_capacity(estimated_size);

    article_html.push_str(
        &HEADER
            .replace("{{TITLE}}", "Tags")
            .replace("{{DESCRIPTION}}", "Page for tagged articles"),
    );
    article_html.push_str("<h1>Tags</h1>\n<hr>\n");

    for (tag, labels) in tags_map {
        writeln!(
            article_html,
            "<h3 id=\"{tag}\"><a href=\"#{tag}\"><em>{tag}</em></a></h3>"
        )?;

        article_html.push_str("<ul>\n");
        for label in labels {
            writeln!(article_html, "<li>{}</li>", label)?;
        }
        article_html.push_str("</ul>\n<hr>\n");
    }

    article_html.push_str(FOOTER);
    fs::write(tags_path, article_html)
        .with_context(|| format!("Failed to write tags file: {}", tags_path.display()))?;

    Ok(())
}
