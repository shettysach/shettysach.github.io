use anyhow::{Context, Result};
use pulldown_cmark::{Event, Parser, html};
use std::{collections::HashMap, fmt::Write, fs, path::Path, rc::Rc};
use syntect::parsing::SyntaxSet;
use walkdir::WalkDir;

use crate::syntex::{OPTIONS, Syntex, extract_metadata};

const HEADER: &str = include_str!("../layout/header.html");
const FOOTER: &str = include_str!("../layout/footer.html");

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
    fn header(&self, url: &str) -> String {
        HEADER
            .replace("{{TITLE}}", &self.title)
            .replace(
                "{{DESCRIPTION}}",
                self.subtitle.as_deref().unwrap_or("Blogpost"),
            )
            .replace(
                "{{TAGS}}",
                &self
                    .tags
                    .as_ref()
                    .map(|tags| tags.join(", "))
                    .unwrap_or_else(|| "blog, blogpost".to_string()),
            )
            .replace("{{URL}}", url)
    }

    fn label(&self, link: &Path) -> String {
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
}

fn render_markdown(
    src: &Path,
    dst: &Path,
    html_dir: &Path,
    syntax_set: &SyntaxSet,
) -> Result<Metadata> {
    let markdown = fs::read_to_string(src)?;

    if dst.exists() && dst.metadata()?.modified()? > src.metadata()?.modified()? {
        return extract_metadata(&markdown);
    }

    let Article { events, metadata } = Parser::new_ext(&markdown, OPTIONS).process(syntax_set)?;

    let rel_path = dst.strip_prefix(html_dir).unwrap().to_string_lossy();

    let md_len = markdown.len();
    let est_size = HEADER.len() + FOOTER.len() + md_len + (md_len >> 1);
    let mut page = String::with_capacity(est_size);

    page.push_str(&metadata.header(&rel_path));
    html::push_html(&mut page, events.into_iter());
    page.push_str(FOOTER);

    fs::write(dst, page)?;

    Ok(metadata)
}

pub(crate) fn index_page(markdown_dir: &Path, html_dir: &Path) -> Result<()> {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let mut tags_map: HashMap<String, Vec<Rc<String>>> = HashMap::new();

    let index_md = fs::read_to_string(markdown_dir.join("index.md"))?;
    let Article { metadata, events } = Parser::new_ext(&index_md, OPTIONS).process(&syntax_set)?;

    let md_len = index_md.len();
    let est_size = HEADER.len() + FOOTER.len() + md_len + (md_len >> 1);
    let mut index_html = String::with_capacity(est_size);

    index_html.push_str(&metadata.header(""));
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
            let metadata = render_markdown(src_path, &html_dst, html_dir, &syntax_set)?;
            let link = rel_path.with_extension("html");

            let label_rc = Rc::new(metadata.label(&link));

            let mut label = (*label_rc).clone();
            if let Some(ref tags) = metadata.tags {
                label.push_str("<br>");
                label.push_str(
                    &tags
                        .iter()
                        .map(|tag| format!("<a href=\"tags.html#{tag}\"><em>{tag}</em></a>"))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }

            if let Some(tags) = metadata.tags {
                for tag in tags {
                    tags_map.entry(tag).or_default().push(Rc::clone(&label_rc));
                }
            }

            writeln!(index_html, "<li>{}</li>", label)?;
        } else if src_path.is_dir() {
            fs::create_dir_all(&dst_path)?
        } else {
            fs::copy(src_path, dst_path)?;
        }
    }

    index_html.push_str("</ul>\n");
    index_html.push_str(FOOTER);

    fs::write(html_dir.join("index.html"), index_html)
        .with_context(|| "Failed to write index.html")?;

    tags_page(tags_map, &html_dir.join("tags.html"))?;

    Ok(())
}

fn tags_page(tags_map: HashMap<String, Vec<Rc<String>>>, tags_path: &Path) -> Result<()> {
    let mut tags: Vec<&String> = tags_map.keys().collect();
    tags.sort();

    let est_size = HEADER.len() + FOOTER.len() + tags.len() * 200;
    let mut article_html = String::with_capacity(est_size);

    article_html.push_str(
        &HEADER
            .replace("{{TITLE}}", "Tags | Sachith Shetty")
            .replace("{{DESCRIPTION}}", "Page for tags and tagged articles")
            .replace("{{TAGS}}", "blog, blogpost, tags")
            .replace("{{URL}}", "tags.html"),
    );

    for tag in tags {
        let labels = &tags_map[tag];
        writeln!(
            article_html,
            "<h1 id=\"{tag}\"><a href=\"#{tag}\"><em>{tag}</em></a></h1>"
        )?;

        article_html.push_str("<ul>\n");
        for label in labels {
            writeln!(article_html, "<li>{}</li>", label)?;
        }
        article_html.push_str("</ul>\n<br><hr>\n");
    }

    article_html.push_str(FOOTER);
    fs::write(tags_path, article_html)?;

    Ok(())
}
