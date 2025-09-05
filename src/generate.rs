use anyhow::{Result, anyhow};
use pulldown_cmark::{Options, Parser, html};
use std::{collections::BTreeMap, fmt::Write, fs, path::Path};
use syntect::parsing::SyntaxSet;
use walkdir::WalkDir;

use crate::syntex::{Article, Metadata, Syntex};

const HEADER: &str = include_str!("../layout/header.html");
const FOOTER: &str = include_str!("../layout/footer.html");

const OPTIONS: Options = Options::empty()
    .union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS)
    .union(Options::ENABLE_MATH)
    .union(Options::ENABLE_HEADING_ATTRIBUTES);

fn render_markdown(src: &Path, dst: &Path, syntax_set: &SyntaxSet) -> Result<Metadata> {
    let markdown = fs::read_to_string(src)?;
    let Article { events, metadata } = Parser::new_ext(&markdown, OPTIONS)
        .process(syntax_set)
        .map_err(|e| anyhow!("{}: {}", e, src.display()))?;

    let mut page = String::with_capacity(markdown.len() * 3 / 2);
    page.push_str(HEADER);
    html::push_html(&mut page, events.into_iter());
    page.push_str(FOOTER);

    fs::write(dst, page)?;
    Ok(metadata)
}

pub(crate) fn index_page(markdown_dir: &Path, html_dir: &Path) -> Result<()> {
    let syntax_set = SyntaxSet::load_defaults_newlines(); // TODO: Lazy/Once init

    let mut tags_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    let index_md = fs::read_to_string(markdown_dir.join("index.md"))?;
    let index_events = Parser::new_ext(&index_md, OPTIONS)
        .process(&syntax_set)?
        .events
        .into_iter();

    let mut index_html = String::from(HEADER);
    // index_html.push_str(HEADER);
    html::push_html(&mut index_html, index_events);
    index_html.push_str("<ul>\n");

    for entry in WalkDir::new(markdown_dir)
        .max_depth(2)
        .sort_by_file_name()
        .into_iter()
        .flatten()
    {
        let src_path = entry.path();
        let rel_path = src_path.strip_prefix(markdown_dir)?;
        let dst_path = html_dir.join(rel_path);

        if src_path.extension().is_some_and(|ext| ext == "md") {
            if src_path.file_name().is_some_and(|i| i == "index.md") {
                continue;
            }

            let dst_path = dst_path.with_extension("html");
            let Metadata {
                title,
                subtitle,
                tags,
            } = render_markdown(src_path, &dst_path, &syntax_set)?;

            let link = rel_path.with_extension("html");
            let mut label = format!(
                "<h2><a href=\"{}\">{}</a></h2>",
                link.to_string_lossy(),
                title
            );
            if let Some(ref subtitle) = subtitle {
                label.push_str(subtitle)
            }

            // TODO: clean up implementation
            if let Some(tags) = tags {
                if subtitle.is_some() {
                    writeln!(label, "<br>")?
                };
                let label_clone = label.clone(); // NOTE clone
                for tag in tags {
                    writeln!(label, "<a href=\"tags.html#{tag}\"><em>{tag}</em></a>, ",)?;
                    tags_map.entry(tag).or_default().push(label_clone.clone()); // NOTE clone
                }
            }

            writeln!(index_html, "<li>{label}</li>")?;
        } else if src_path.is_dir() {
            fs::create_dir_all(dst_path)?; // Create parent directiories
        } else {
            fs::copy(src_path, dst_path)?; // Copy assets
        }
    }

    index_html.push_str("</ul>\n");
    index_html.push_str(FOOTER);

    fs::write(html_dir.join("index.html"), index_html)?;
    tags_page(tags_map, &html_dir.join("tags.html"))?;

    Ok(())
}

fn tags_page(tags_map: BTreeMap<String, Vec<String>>, tags_path: &Path) -> Result<()> {
    let mut article_html = String::from(HEADER);
    article_html.push_str("<h1>Tags</h1>\n<hr>\n");

    for (tag, labels) in tags_map {
        writeln!(
            article_html,
            "<h2 id=\"{tag}\"><a href=\"#{tag}\"><em>{tag}</em></a></h2>"
        )?;

        article_html.push_str("<ul>\n");
        for label in labels {
            writeln!(article_html, "<li>{label}</li>")?;
        }
        article_html.push_str("</ul>\n<hr>\n");
    }

    article_html.push_str(FOOTER);
    fs::write(tags_path, article_html)?;

    Ok(())
}
