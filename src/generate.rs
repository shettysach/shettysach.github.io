use crate::{
    atom::{generate_atom_feed, Atom},
    syntex::{extract_metadata, Article, Metadata, Syntex, OPTIONS},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use pulldown_cmark::{html::push_html, Parser};
use std::{collections::HashMap, fmt::Write, fs, path::Path, rc::Rc, time::SystemTime};
use walkdir::WalkDir;

const HEADER: &str = include_str!("../layout/header.html");
const FOOTER: &str = include_str!("../layout/footer.html");

fn render_markdown(
    src: &Path,
    dst: &Path,
    modified: SystemTime,
    rel_url: &str,
) -> Result<Metadata> {
    let markdown = fs::read_to_string(src)?;

    if dst.exists() && dst.metadata()?.modified()? > modified {
        return extract_metadata(&markdown)
            .with_context(|| format!("Invalid frontmatter, {}", src.to_string_lossy()));
    }

    let Article {
        events,
        metadata,
        toc,
    } = Parser::new_ext(&markdown, OPTIONS).process()?;

    let md_len = markdown.len();
    let est_size = HEADER.len() + FOOTER.len() + md_len + (md_len >> 1);
    let mut html = String::with_capacity(est_size);

    html.push_str(&metadata.header(rel_url));

    if let Some(table) = toc {
        let table = Parser::new(&table);
        html.push_str("<details><summary> Table of contents </summary>");
        push_html(&mut html, table);
        html.push_str("</details>");
    }

    push_html(&mut html, events.into_iter());
    html.push_str(FOOTER);

    fs::write(dst, html)?;

    Ok(metadata)
}

pub(crate) fn index_page(markdown_dir: &Path, html_dir: &Path) -> Result<()> {
    let index_md = fs::read_to_string(markdown_dir.join("index.md"))?;
    let Article {
        metadata, events, ..
    } = Parser::new_ext(&index_md, OPTIONS).process()?;

    let md_len = index_md.len();
    let est_size = HEADER.len() + FOOTER.len() + md_len + (md_len >> 1);
    let mut index_html = String::with_capacity(est_size);

    index_html.push_str(&metadata.header(""));
    push_html(&mut index_html, events.into_iter());
    index_html.push_str("<ul>\n");

    // TODO: Estimate no. of tags
    let mut tags_map: HashMap<String, Vec<Rc<String>>> = HashMap::with_capacity(10);

    // NOTE: Estimate, cases - same dir 2mds
    let est_count = WalkDir::new(markdown_dir).max_depth(1).into_iter().count() - 4;
    let mut atom_entries = Vec::with_capacity(est_count);

    for entry in WalkDir::new(markdown_dir)
        .max_depth(2)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_name() != "index.md")
    {
        let src_path = entry.path();
        let rel_path = src_path.strip_prefix(markdown_dir)?;
        let dst_path = html_dir.join(rel_path);

        if src_path.extension().is_some_and(|ext| ext == "md") {
            let dst_html = dst_path.with_extension("html");
            let modified = src_path.metadata()?.modified()?;
            let rel_html = rel_path.with_extension("html");
            let rel_url = rel_html.to_str().with_context(|| "Path not UTF8")?;

            let metadata = render_markdown(src_path, &dst_html, modified, rel_url)?;
            let label_rc = Rc::new(metadata.label(rel_url));

            let mut label = (*label_rc).clone();

            if let Some(tags) = metadata.tags {
                label.push_str("<br>");
                label.push_str(
                    &tags
                        .iter()
                        .map(|tag| format!("<a href=\"tags.html#{tag}\"><em>{tag}</em></a>"))
                        .collect::<Vec<String>>()
                        .join(", "),
                );

                for tag in tags {
                    tags_map.entry(tag).or_default().push(label_rc.clone());
                }
            }

            writeln!(index_html, "<li>{}</li>", label)?;

            atom_entries.push(Atom {
                title: metadata.title,
                subtitle: metadata.subtitle,
                datetime: DateTime::<Utc>::from(modified),
                url: rel_url.to_string(),
            });
        } else if src_path.is_dir() {
            fs::create_dir_all(&dst_path)?
        } else {
            fs::copy(src_path, dst_path)?;
        }
    }

    index_html.push_str("</ul>\n");
    index_html.push_str(FOOTER);

    fs::write(html_dir.join("index.html"), index_html)?;

    tags_page(tags_map, &html_dir.join("tags.html"))?;

    generate_atom_feed(atom_entries, &html_dir.join("atom.xml"))?;

    Ok(())
}

fn tags_page(tags_map: HashMap<String, Vec<Rc<String>>>, tags_path: &Path) -> Result<()> {
    let mut tags: Vec<&String> = tags_map.keys().collect();
    tags.sort();

    let est_size = HEADER.len() + FOOTER.len() + tags.len() * 200;
    let mut article_html = String::with_capacity(est_size);

    article_html.push_str(
        &HEADER
            .replace("{{TITLE}}", "Tags")
            .replace("{{DESCRIPTION}}", "Page for tags and tagged articles")
            .replace("{{TAGS}}", "blog, blogpost, tags")
            .replace("{{URL}}", "tags.html"),
    );

    for tag in tags {
        let labels = &tags_map[tag];
        writeln!(article_html, "<h1 id=\"{tag}\"><em>{tag}</em></h1>")?;

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

impl Metadata {
    fn header(&self, url: &str) -> String {
        let description = self.subtitle.as_deref().unwrap_or("Blogpost");
        let tags = &self
            .tags
            .as_ref()
            .map(|tags| tags.join(", ")) // NOTE: Use CoW? negligible?
            .unwrap_or_else(|| "blog, blogpost, article".to_string());

        HEADER
            .replace("{{TITLE}}", &self.title)
            .replace("{{DESCRIPTION}}", description)
            .replace("{{TAGS}}", tags)
            .replace("{{URL}}", url)
    }

    fn label(&self, link: &str) -> String {
        let mut label = format!("<h2><a href=\"{}\">{}</a></h2>", link, self.title);

        if let Some(subtitle) = &self.subtitle {
            label.push_str(subtitle);
        }

        label
    }
}
