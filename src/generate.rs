use crate::{
    anchored::AnchoredIterator,
    syntex::{CustomIterator, OPTIONS, process_metadata},
    xml::{Entry, generate_feed, generate_sitemap},
};
use anyhow::{Result, anyhow};
use chrono::{DateTime, NaiveDate, Utc};
use pulldown_cmark::{Parser, TextMergeStream, html::write_html_io};
use std::{
    collections::HashMap,
    fs,
    io::{BufWriter, Write},
    path::Path,
    time::SystemTime,
};
use walkdir::WalkDir;
use yaml_rust2::Yaml;

const HEADER: &str = include_str!("../layout/header.html");
const FOOTER: &str = include_str!("../layout/footer.html");

pub(crate) fn ssgenerate(markdown_dir: &Path, html_dir: &Path) -> Result<()> {
    generate_index(markdown_dir, html_dir)?;

    let (atom_entries, labels, tags_map) = collect_articles(markdown_dir, html_dir)?;

    generate_articles(&labels, html_dir)?;
    generate_tags(&labels, tags_map, html_dir)?;
    generate_feed(&atom_entries, &html_dir.join("atom.xml"))?;
    generate_sitemap(atom_entries, &html_dir.join("sitemap.xml"))?;

    Ok(())
}

fn generate_index(markdown_dir: &Path, html_dir: &Path) -> Result<()> {
    let index_md = fs::read_to_string(markdown_dir.join("index.md"))?;
    let mut parser = Parser::new_ext(&index_md, OPTIONS);

    let (frontmatter, _, _) = process_metadata(&mut parser)
        .and_then(Frontmatter::parse_metadata)
        .ok_or_else(|| anyhow!("YAML Frontmatter error at index.md"))?;

    let file = fs::File::create(html_dir.join("index.html"))?;
    let mut writer = BufWriter::new(file);

    writer.write_all(frontmatter.generate_header("").as_bytes())?;

    // let parser = CustomIterator::new(parser);
    write_html_io(&mut writer, parser)?;

    writer.write_all(FOOTER.as_bytes())?;
    writer.flush()?;

    Ok(())
}

type TagsMap = HashMap<String, (Vec<usize>, SystemTime)>;
fn collect_articles(
    markdown_dir: &Path,
    html_dir: &Path,
) -> Result<(Vec<Entry>, Vec<String>, TagsMap)> {
    let mut articles = Vec::new();
    let mut tags_map: TagsMap = HashMap::new();

    let index_path = markdown_dir.join("index.md");
    for entry in WalkDir::new(markdown_dir)
        .max_depth(2)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path() != index_path)
    {
        let src_path = entry.path();
        let rel_path = src_path.strip_prefix(markdown_dir)?;
        let dst_path = html_dir.join(rel_path);

        if src_path.extension().is_some_and(|ext| ext == "md") {
            let dst_html = dst_path.with_extension("html");
            let modified = src_path.metadata()?.modified()?;
            let rel_url = rel_path
                .with_extension("html")
                .into_os_string()
                .into_string()
                .map_err(|_| anyhow!("Path not UTF8"))?;

            let (frontmatter, date) = generate_article(src_path, &dst_html, modified, &rel_url)?;

            if frontmatter.draft {
                continue;
            }

            let datetime = date.map_or_else(
                || DateTime::<Utc>::from(modified),
                |nd| nd.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            );

            articles.push((frontmatter, datetime, rel_url, modified));
        } else if src_path.is_dir() {
            if !dst_path.exists() {
                fs::create_dir(&dst_path)?;
            }
        } else {
            fs::copy(src_path, dst_path)?;
        }
    }

    // Sort by date, newest first
    articles.sort_by(|a, b| b.1.cmp(&a.1));

    let (entries, labels): (Vec<Entry>, Vec<String>) = articles
        .into_iter()
        .enumerate()
        .map(|(idx, (frontmatter, datetime, rel_url, modified))| {
            let mut label = frontmatter.label(&rel_url, &datetime);

            if let Some(tags) = frontmatter.tags {
                label.push_str(" · ");

                let mut first = true;
                for tag in tags {
                    if !first {
                        label.push_str(", ");
                    }
                    first = false;
                    label.push_str("<a href=\"");
                    label.push_str(&tag);
                    label.push_str(".html\">");
                    label.push_str(&tag);
                    label.push_str("</a>");

                    tags_map
                        .entry(tag)
                        .and_modify(|(indices, max_mod)| {
                            indices.push(idx);
                            if modified > *max_mod {
                                *max_mod = modified;
                            }
                        })
                        .or_insert_with(|| (vec![idx], modified));
                }
            }

            (
                Entry {
                    title: frontmatter.title,
                    subtitle: frontmatter.subtitle,
                    datetime,
                    rel_url,
                },
                label,
            )
        })
        .unzip();

    Ok((entries, labels, tags_map))
}

fn generate_article(
    src: &Path,
    dst: &Path,
    modified: SystemTime,
    rel_url: &str,
) -> Result<(Frontmatter, Option<NaiveDate>)> {
    let markdown = fs::read_to_string(src)?;
    let mut parser = Parser::new_ext(&markdown, OPTIONS);

    let (frontmatter, date, anchors) = process_metadata(&mut parser)
        .and_then(Frontmatter::parse_metadata)
        .ok_or_else(|| anyhow!("YAML Frontmatter error {}", src.to_string_lossy()))?;

    if dst.exists() && dst.metadata()?.modified()? > modified {
        return Ok((frontmatter, date));
    }

    let file = fs::File::create(dst)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(frontmatter.generate_header(rel_url).as_bytes())?;

    let parser = TextMergeStream::new(parser);
    if anchors {
        let parser = AnchoredIterator::new(parser);
        write_html_io(&mut writer, parser)?;
    } else {
        let parser = CustomIterator::new(parser);
        write_html_io(&mut writer, parser)?;
    }

    writer.write_all(FOOTER.as_bytes())?;
    writer.flush()?;

    Ok((frontmatter, date))
}

fn generate_articles(labels: &[String], html_dir: &Path) -> Result<()> {
    let file = fs::File::create(html_dir.join("articles.html"))?;
    let mut writer = BufWriter::new(file);

    let header_str = generate_header(
        "Articles",
        "List of all articles",
        "blog, blogpost, articles",
        "articles.html",
    );

    writer.write_all(header_str.as_bytes())?;
    writer.write_all("<h1>Articles</h1><hr>".as_bytes())?;

    writer.write_all(b"<ul>\n")?;
    for label in labels {
        writer.write_all(b"<li>")?;
        writer.write_all(label.as_bytes())?;
        writer.write_all(b"</li>\n")?;
    }
    writer.write_all(b"</ul>\n")?;
    writer.write_all(FOOTER.as_bytes())?;
    writer.flush()?;

    Ok(())
}

fn generate_tags(labels: &[String], tags_map: TagsMap, html_dir: &Path) -> Result<()> {
    let mut tags: Vec<&String> = tags_map.keys().collect();
    tags.sort();

    let file = fs::File::create(html_dir.join("tags.html"))?;
    let mut writer = BufWriter::new(file);

    let header_str = generate_header(
        "Tags",
        "Page for tags and tagged articles",
        "blog, blogpost, tags",
        "tags.html",
    );
    writer.write_all(header_str.as_bytes())?;
    writer.write_all(b"<h1>Tags</h1><hr>")?;

    writer.write_all(b"<ul>\n")?;
    for tag in &tags {
        writer.write_all(b"<li><a href=\"")?;
        writer.write_all(tag.as_bytes())?;
        writer.write_all(b".html\">")?;
        writer.write_all(tag.as_bytes())?;
        writer.write_all(b"</a></li>\n")?;
    }
    writer.write_all(b"</ul>\n")?;

    writer.write_all(FOOTER.as_bytes())?;
    writer.flush()?;

    // Generate individual tag pages
    for tag in &tags {
        let (indices, src_modified) = &tags_map[*tag];
        let dst_path = html_dir.join(format!("{tag}.html"));

        if dst_path.exists() && dst_path.metadata()?.modified()? > *src_modified {
            continue;
        }

        let file = fs::File::create(dst_path)?;
        let mut writer = BufWriter::new(file);

        let header_str = generate_header(
            tag,
            &format!("Articles tagged {tag}"),
            &format!("blog, blogpost, {tag}"),
            &format!("{tag}.html"),
        );
        writer.write_all(header_str.as_bytes())?;
        writer.write_all(b"<h1>")?;
        writer.write_all(tag.as_bytes())?;
        writer.write_all(b"</h1><hr>")?;

        writer.write_all(b"<ul>\n")?;
        for &idx in indices {
            writer.write_all(b"<li>")?;
            writer.write_all(labels[idx].as_bytes())?;
            writer.write_all(b"</li>\n")?;
        }
        writer.write_all(b"</ul>\n")?;

        writer.write_all(FOOTER.as_bytes())?;
        writer.flush()?;
    }

    Ok(())
}

fn generate_header(title: &str, description: &str, tags: &str, url: &str) -> String {
    // NOTE: performs 4 allocs, negligible
    // or better to use templating engine ?
    HEADER
        .replace("{{TITLE}}", title)
        .replace("{{DESCRIPTION}}", description)
        .replace("{{TAGS}}", tags)
        .replace("{{URL}}", url)
}

pub(crate) struct Frontmatter {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) tags: Option<Vec<String>>,
    pub(crate) draft: bool,
}

impl Frontmatter {
    fn generate_header(&self, url: &str) -> String {
        let description = self.subtitle.as_deref().unwrap_or("Blogpost");
        let tags = &self
            .tags
            .as_ref()
            .map(|tags| tags.join(", "))
            .unwrap_or_else(|| "blog, blogpost, article".to_string());

        generate_header(&self.title, description, tags, url)
    }

    fn label(&self, link: &str, date: &DateTime<Utc>) -> String {
        let mut label = format!(
            "<h2 class=\"header-link\"><a href=\"{}\">{}</a></h2>",
            link, self.title
        );

        if let Some(subtitle) = &self.subtitle {
            label.push_str(subtitle);
            label.push_str("<br>");
        }

        label.push_str("<small class=\"article-date\">");
        label.push_str(&date.format("%B %d, %Y").to_string());
        label.push_str("</small>");

        label
    }

    fn parse_metadata(doc: Yaml) -> Option<(Frontmatter, Option<NaiveDate>, bool)> {
        let title = doc["title"].as_str()?.to_string();
        let subtitle = doc["subtitle"].as_str().map(str::to_string);
        let tags = doc["tags"]
            .as_vec()
            .and_then(|vec| vec.iter().map(|v| v.as_str().map(str::to_string)).collect());
        let draft = doc["draft"].as_bool().unwrap_or(false);
        let anchors = doc["anchors"].as_bool().unwrap_or(false);
        let date = doc["date"]
            .as_str()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        Some((
            Frontmatter {
                title,
                subtitle,
                tags,
                draft,
            },
            date,
            anchors,
        ))
    }
}
