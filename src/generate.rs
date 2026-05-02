use crate::{
    anchored::AnchoredIterator,
    cache::{CacheEntry, MetadataCache},
    syntex::{CustomIterator, OPTIONS, process_metadata},
    utils::{html_escape, slugify},
    xml::{Entry, generate_atom_feed, generate_sitemap},
};
use anyhow::{Result, anyhow};
use chrono::{DateTime, NaiveDate, Utc};
use pulldown_cmark::{Parser, TextMergeStream, html::write_html_io};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

const HEADER: &str = include_str!("../layout/header.html");
const FOOTER: &str = include_str!("../layout/footer.html");
const EXPECT: [&str; 5] = [
    "index.html",
    "articles.html",
    "tags.html",
    "atom.xml",
    "sitemap.xml",
];

pub(crate) fn generate_site(
    input_dir: &Path,
    output_dir: &Path,
    cache: &mut MetadataCache,
) -> Result<()> {
    generate_index(input_dir, output_dir, cache)?;

    let mut expected = HashSet::from_iter(EXPECT.into_iter().map(|f| output_dir.join(f)));
    let (labels, entries, tags_map) =
        collect_articles(input_dir, output_dir, cache, &mut expected)?;

    generate_articles_page(&labels, output_dir)?;
    generate_tags_page(&labels, tags_map, output_dir, &mut expected)?;
    generate_atom_feed(&entries, &output_dir.join("atom.xml"))?;
    generate_sitemap(entries, &output_dir.join("sitemap.xml"))?;

    cleanup_stale(output_dir, expected)
}

fn generate_index(input_dir: &Path, output_dir: &Path, cache: &mut MetadataCache) -> Result<()> {
    let src_path = input_dir.join("index.md");
    let modified = src_path.metadata()?.modified()?;

    if let Some(entry) = cache.entries.get("index.md")
        && entry.mtime >= modified
    {
        return Ok(());
    }

    let index_md = fs::read_to_string(&src_path)?;
    let mut parser = Parser::new_ext(&index_md, OPTIONS);

    let metadata = process_metadata(&mut parser)
        .and_then(Metadata::parse_metadata)
        .ok_or_else(|| anyhow!("YAML Frontmatter error at index.md"))?;

    let file = fs::File::create(output_dir.join("index.html"))?;
    let mut writer = BufWriter::new(file);

    writer.write_all(metadata.generate_header("").as_bytes())?;

    write_html_io(&mut writer, parser)?;

    writer.write_all(FOOTER.as_bytes())?;
    writer.flush()?;

    cache.entries.insert(
        "index.md".to_string(),
        CacheEntry {
            mtime: modified,
            metadata,
        },
    );

    Ok(())
}

type TagsMap = HashMap<String, (Vec<usize>, SystemTime)>;

fn collect_articles(
    input_dir: &Path,
    output_dir: &Path,
    cache: &mut MetadataCache,
    expected: &mut HashSet<PathBuf>,
) -> Result<(Vec<String>, Vec<Entry>, TagsMap)> {
    let mut articles = Vec::new();
    let mut tags_map: TagsMap = HashMap::new();

    let index_path = input_dir.join("index.md");
    for entry in walkdir::WalkDir::new(input_dir)
        .max_depth(2)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path() != index_path)
    {
        let src_path = entry.path();
        let rel_path = src_path.strip_prefix(input_dir)?;
        let dst_path = output_dir.join(rel_path);

        if src_path.extension().is_some_and(|ext| ext == "md") {
            let dst_html = dst_path.with_extension("html");
            expected.insert(dst_html.clone());
            let modified = src_path.metadata()?.modified()?;
            let rel_path_str = rel_path.to_str().ok_or_else(|| anyhow!("Path not UTF8"))?;
            let rel_url = rel_path
                .with_extension("html")
                .to_str()
                .ok_or_else(|| anyhow!("Path not UTF8"))?
                .to_string();

            let metadata =
                generate_article(src_path, &dst_html, modified, &rel_url, rel_path_str, cache)?;

            if metadata.draft {
                continue;
            }

            let datetime = metadata.date.map_or_else(
                || DateTime::<Utc>::from(modified),
                |nd| nd.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            );

            articles.push((metadata, datetime, rel_url, modified));
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

    let (labels, entries): (Vec<String>, Vec<Entry>) = articles
        .into_iter()
        .enumerate()
        .map(|(idx, (metadata, datetime, rel_url, modified))| {
            let mut label = metadata.label(&rel_url, &datetime);

            if let Some(tags) = metadata.tags {
                label.push_str(" · ");

                let mut first = true;
                for tag in tags {
                    if !first {
                        label.push_str(", ");
                    }
                    first = false;
                    let slug = slugify(&tag);
                    let escaped = html_escape(&tag);
                    label.push_str("<a href=\"");
                    label.push_str(&slug);
                    label.push_str(".html\">");
                    label.push_str(&escaped);
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
                label,
                Entry {
                    title: metadata.title,
                    subtitle: metadata.subtitle,
                    datetime,
                    rel_url,
                },
            )
        })
        .unzip();

    Ok((labels, entries, tags_map))
}

fn generate_article(
    src: &Path,
    dst: &Path,
    modified: SystemTime,
    rel_url: &str,
    rel_path: &str,
    cache: &mut MetadataCache,
) -> Result<Metadata> {
    // Cache hit. No change article, no need to render.
    if let Some(entry) = cache.entries.get(rel_path)
        && entry.mtime >= modified
    {
        return Ok(entry.metadata.clone());
    }

    let markdown = fs::read_to_string(src)?;
    let mut parser = Parser::new_ext(&markdown, OPTIONS);

    let doc = process_metadata(&mut parser)
        .ok_or_else(|| anyhow!("YAML Frontmatter error {}", src.display()))?;
    let anchors = doc["anchors"].as_bool().unwrap_or(false);
    let metadata = Metadata::parse_metadata(doc)
        .ok_or_else(|| anyhow!("YAML Frontmatter error {}", src.display()))?;

    // Cache miss, but destination HTML is already up-to-date.
    // Skip rendering and repopulate cache for next time.
    if dst.exists() && dst.metadata()?.modified()? > modified {
        cache.entries.insert(
            rel_path.to_string(),
            CacheEntry {
                mtime: modified,
                metadata: metadata.clone(),
            },
        );
        return Ok(metadata);
    }

    let file = fs::File::create(dst)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(metadata.generate_header(rel_url).as_bytes())?;

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

    cache.entries.insert(
        rel_path.to_string(),
        CacheEntry {
            mtime: modified,
            metadata: metadata.clone(),
        },
    );
    Ok(metadata)
}

fn generate_articles_page(labels: &[String], output_dir: &Path) -> Result<()> {
    let file = fs::File::create(output_dir.join("articles.html"))?;
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

fn generate_tags_page(
    labels: &[String],
    tags_map: TagsMap,
    output_dir: &Path,
    expected: &mut HashSet<PathBuf>,
) -> Result<()> {
    let mut tags: Vec<&String> = tags_map.keys().collect();
    tags.sort();

    let file = fs::File::create(output_dir.join("tags.html"))?;
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
        let slug = slugify(tag);
        let escaped = html_escape(tag);
        writer.write_all(br#"<li><a href=""#)?;
        writer.write_all(slug.as_bytes())?;
        writer.write_all(br#".html">"#)?;
        writer.write_all(escaped.as_bytes())?;
        writer.write_all(b"</a></li>\n")?;
    }
    writer.write_all(b"</ul>\n")?;

    writer.write_all(FOOTER.as_bytes())?;
    writer.flush()?;

    // Generate individual tag pages
    for tag in &tags {
        let (indices, src_modified) = &tags_map[*tag];
        let slug = slugify(tag);
        let escaped = html_escape(tag);
        let dst_path = output_dir.join(&slug).with_extension("html");
        expected.insert(dst_path.clone());

        if dst_path.exists() && dst_path.metadata()?.modified()? > *src_modified {
            continue;
        }

        let file = fs::File::create(dst_path)?;
        let mut writer = BufWriter::new(file);

        let header_str = generate_header(
            &escaped,
            &format!("Articles tagged {escaped}"),
            &format!("blog, blogpost, {escaped}"),
            &format!("{slug}.html"),
        );
        writer.write_all(header_str.as_bytes())?;
        writer.write_all(b"<h1>")?;
        writer.write_all(escaped.as_bytes())?;
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

fn cleanup_stale(output_dir: &Path, expected: HashSet<PathBuf>) -> Result<()> {
    for entry in walkdir::WalkDir::new(output_dir).into_iter().flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("html") && ext != Some("xml") {
            continue;
        }
        if !expected.contains(path) {
            fs::remove_file(path)?
        }
    }

    Ok(())
}

fn generate_header(title: &str, description: &str, tags: &str, url: &str) -> String {
    HEADER
        .replace("{{TITLE}}", title)
        .replace("{{DESCRIPTION}}", description)
        .replace("{{TAGS}}", tags)
        .replace("{{URL}}", url)
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Metadata {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) tags: Option<Vec<String>>,
    pub(crate) draft: bool,
    pub(crate) date: Option<NaiveDate>,
}

impl Metadata {
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

    fn parse_metadata(doc: yaml_rust2::Yaml) -> Option<Metadata> {
        let title = doc["title"].as_str()?.to_string();
        let subtitle = doc["subtitle"].as_str().map(str::to_string);
        let tags = doc["tags"]
            .as_vec()
            .and_then(|vec| vec.iter().map(|v| v.as_str().map(str::to_string)).collect());
        let draft = doc["draft"].as_bool().unwrap_or(false);
        let date = doc["date"]
            .as_str()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        Some(Metadata {
            title,
            subtitle,
            tags,
            draft,
            date,
        })
    }
}
