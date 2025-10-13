use crate::{
    atom::{Atom, generate_atom_feed},
    syntex::{Article, Custom, Frontmatter, OPTIONS, extract_metadata},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use pulldown_cmark::{Parser, html::write_html_io};
use std::io::{BufWriter, Write};
use std::{collections::HashMap, fs, path::Path, rc::Rc, time::SystemTime};
use walkdir::WalkDir;

const HEADER: &str = include_str!("../layout/header.html");
const FOOTER: &str = include_str!("../layout/footer.html");

fn render_markdown(
    src: &Path,
    dst: &Path,
    modified: SystemTime,
    rel_url: &str,
) -> Result<Frontmatter> {
    let markdown = fs::read_to_string(src)?;

    if dst.exists() && dst.metadata()?.modified()? > modified {
        return extract_metadata(&markdown)
            .with_context(|| format!("Invalid frontmatter, {}", src.to_string_lossy()));
    }

    let Article {
        events,
        frontmatter,
        toc,
    } = Parser::new_ext(&markdown, OPTIONS).process()?;

    let file = fs::File::create(dst)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(frontmatter.header(rel_url).as_bytes())?;

    if let Some(table) = toc {
        let table = Parser::new(&table);
        writer.write_all(b"<details><summary> Table of contents </summary>")?;
        write_html_io(&mut writer, table)?;
        writer.write_all(b"</details>")?;
    }

    write_html_io(&mut writer, events.into_iter())?;
    writer.write_all(FOOTER.as_bytes())?;

    writer.flush()?;

    Ok(frontmatter)
}

pub(crate) fn index_page(markdown_dir: &Path, html_dir: &Path) -> Result<()> {
    let index_md = fs::read_to_string(markdown_dir.join("index.md"))?;
    let Article {
        frontmatter: metadata,
        events,
        ..
    } = Parser::new_ext(&index_md, OPTIONS).process()?;

    let file = fs::File::create(html_dir.join("index.html"))?;
    let mut writer = BufWriter::new(file);

    writer.write_all(metadata.header("").as_bytes())?;
    write_html_io(&mut writer, events.into_iter())?;
    writer.write_all(b"<ul>\n")?;

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
        .filter(|e| e.file_name() != "index.md" && e.path() != markdown_dir)
    {
        let src_path = entry.path();
        let rel_path = src_path.strip_prefix(markdown_dir)?;
        let dst_path = html_dir.join(rel_path);

        if src_path.extension().is_some_and(|ext| ext == "md") {
            let dst_html = dst_path.with_extension("html");
            let modified = src_path.metadata()?.modified()?;
            let rel_html = rel_path.with_extension("html");
            let rel_url = rel_html.to_str().with_context(|| "Path not UTF8")?;

            let frontmatter = render_markdown(src_path, &dst_html, modified, rel_url)?;
            let label_rc = Rc::new(frontmatter.label(rel_url));

            let mut label = (*label_rc).clone();

            if let Some(tags) = frontmatter.tags {
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

            writer.write_all(format!("<li>{}</li>\n", label).as_bytes())?;

            atom_entries.push(Atom {
                title: frontmatter.title,
                subtitle: frontmatter.subtitle,
                datetime: DateTime::<Utc>::from(modified),
                url: rel_url.to_string(),
            });
        } else if src_path.is_dir() {
            if !dst_path.exists() {
                fs::create_dir(&dst_path)?;
            }
        } else {
            fs::copy(src_path, dst_path)?;
        }
    }

    writer.write_all(b"</ul>\n")?;
    writer.write_all(FOOTER.as_bytes())?;
    writer.flush()?;

    tags_page(tags_map, &html_dir.join("tags.html"))?;

    generate_atom_feed(atom_entries, &html_dir.join("atom.xml"))?;

    Ok(())
}

fn tags_page(tags_map: HashMap<String, Vec<Rc<String>>>, tags_path: &Path) -> Result<()> {
    let mut tags: Vec<&String> = tags_map.keys().collect();
    tags.sort();

    let file = fs::File::create(tags_path)?;
    let mut writer = BufWriter::new(file);

    let header_str = HEADER
        .replace("{{TITLE}}", "Tags")
        .replace("{{DESCRIPTION}}", "Page for tags and tagged articles")
        .replace("{{TAGS}}", "blog, blogpost, tags")
        .replace("{{URL}}", "tags.html");
    writer.write_all(header_str.as_bytes())?;

    for tag in tags {
        let labels = &tags_map[tag];
        writer.write_all(format!("<h1 id=\"{tag}\"><em>{tag}</em></h1>\n").as_bytes())?;

        writer.write_all(b"<ul>\n")?;
        for label in labels {
            writer.write_all(format!("<li>{}</li>\n", label).as_bytes())?;
        }
        writer.write_all(b"</ul>\n<br><hr>\n")?;
    }

    writer.write_all(FOOTER.as_bytes())?;
    writer.flush()?;

    Ok(())
}

impl Frontmatter {
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
