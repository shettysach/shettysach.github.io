use crate::{
    anchored::AnchoredIterator,
    cache::{CacheEntry, MetadataCache},
    syntex::{CustomIterator, OPTIONS, process_metadata},
    utils::{html_escape, slugify},
    xml::{Entry, generate_atom_feed, generate_sitemap},
};
use anyhow::{Ok, Result, anyhow};
use chrono::{DateTime, NaiveDate, Utc};
use pulldown_cmark::{Parser, TextMergeStream, html::write_html_io};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::{BufWriter, Write},
    path::Path,
    time::SystemTime,
};

const HEADER: &str = include_str!("../layout/header.html");
const FOOTER: &str = include_str!("../layout/footer.html");

struct ArticleBuild {
    metadata: Metadata,
    datetime: DateTime<Utc>,
    rel_url: String,
    modified: SystemTime,
}

pub(crate) fn generate_site(
    input_dir: &Path,
    output_dir: &Path,
    cache: &mut MetadataCache,
) -> Result<()> {
    generate_site_index(input_dir, output_dir, cache)?;

    let (articles, article_urls) = process_articles(input_dir, output_dir, cache)?;
    let (labels, entries, tags_map) = generate_article_pages(articles);

    generate_articles_index(&labels, &output_dir.join("articles.html"))?;
    generate_tags_index(&tags_map, &output_dir.join("tags.html"))?;

    let tag_urls = generate_tag_pages(labels, tags_map, output_dir)?;

    generate_atom_feed(&entries, &output_dir.join("atom.xml"))?;
    generate_sitemap(entries, &output_dir.join("sitemap.xml"))?;

    let indexes = ["index.html", "articles.html", "tags.html"];
    let mut retain = HashSet::with_capacity(indexes.len() + article_urls.len() + tag_urls.len());
    retain.extend(indexes.map(str::to_string));
    retain.extend(article_urls);
    retain.extend(tag_urls);

    cache.entries.retain(|path, _| retain.contains(path)); // Prune
    cleanup_stale(output_dir, retain)
}

fn generate_site_index(
    input_dir: &Path,
    output_dir: &Path,
    cache: &mut MetadataCache,
) -> Result<()> {
    let src_path = input_dir.join("index.md");
    let mtime = src_path.metadata()?.modified()?;

    if let Some(entry) = cache.entries.get("index.html")
        && entry.mtime >= mtime
    {
        return Ok(());
    }

    let index_md = fs::read_to_string(&src_path)?;
    let mut parser = Parser::new_ext(&index_md, OPTIONS);

    let metadata = process_metadata(&mut parser)
        .and_then(Metadata::parse_metadata)
        .ok_or_else(|| anyhow!("YAML Frontmatter error at index.md"))?;

    let dst_path = output_dir.join("index.html");
    let mut writer = BufWriter::new(fs::File::create(dst_path)?);

    writer.write_all(metadata.generate_header("").as_bytes())?;
    write_html_io(&mut writer, parser)?;
    writer.write_all(FOOTER.as_bytes())?;
    writer.flush()?;

    cache
        .entries
        .insert("index.html".to_string(), CacheEntry { mtime, metadata });

    Ok(())
}

// NOTE: Replace with Hashmap + Key sort as it scales
type TagsMap = BTreeMap<String, (Vec<usize>, SystemTime)>;

fn process_articles(
    input_dir: &Path,
    output_dir: &Path,
    cache: &mut MetadataCache,
) -> Result<(Vec<ArticleBuild>, Vec<String>)> {
    let mut articles = Vec::new();
    let mut article_urls = Vec::new();

    let index_path = input_dir.join("index.md");
    for entry in walkdir::WalkDir::new(input_dir)
        .max_depth(2)
        .into_iter()
        .flatten()
        .filter(|e| e.path() != index_path)
    {
        let src_path = entry.path();
        let rel_path = src_path.strip_prefix(input_dir)?;
        let dst_path = output_dir.join(rel_path);

        if src_path.extension().is_some_and(|ext| ext == "md") {
            let dst_html = dst_path.with_extension("html");

            let modified = src_path.metadata()?.modified()?;
            let rel_url = rel_path
                .with_extension("html")
                .to_str()
                .ok_or_else(|| anyhow!("Path not UTF8"))?
                .to_string();

            let metadata = render_article(src_path, &dst_html, modified, rel_url.clone(), cache)?;
            article_urls.push(rel_url.clone());

            if metadata.draft {
                continue;
            }

            let datetime = metadata.date.map_or_else(
                || DateTime::<Utc>::from(modified),
                |nd| nd.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            );

            articles.push(ArticleBuild {
                metadata,
                datetime,
                rel_url,
                modified,
            });
        } else if src_path.is_dir() {
            if !dst_path.exists() {
                fs::create_dir(&dst_path)?;
            }
        } else {
            fs::copy(src_path, dst_path)?;
        }
    }

    // Sort by date, newest first
    articles.sort_by(|a, b| b.datetime.cmp(&a.datetime));
    Ok((articles, article_urls))
}

fn generate_article_pages(articles: Vec<ArticleBuild>) -> (Vec<String>, Vec<Entry>, TagsMap) {
    let mut tags_map: TagsMap = BTreeMap::new();

    let (labels, entries): (Vec<String>, Vec<Entry>) = articles
        .into_iter()
        .enumerate()
        .map(
            |(
                idx,
                ArticleBuild {
                    metadata,
                    datetime,
                    rel_url,
                    modified,
                },
            )| {
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
            },
        )
        .unzip();

    (labels, entries, tags_map)
}

fn render_article(
    src: &Path,
    dst: &Path,
    mtime: SystemTime,
    rel_url: String,
    cache: &mut MetadataCache,
) -> Result<Metadata> {
    // Cache hit. No change article, no need to render.
    if let Some(entry) = cache.entries.get(&rel_url)
        && entry.mtime >= mtime
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
    if dst.exists() && dst.metadata()?.modified()? > mtime {
        cache.entries.insert(
            rel_url,
            CacheEntry {
                mtime,
                metadata: metadata.clone(),
            },
        );
        return Ok(metadata);
    }

    let file = fs::File::create(dst)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(metadata.generate_header(&rel_url).as_bytes())?;
    cache.entries.insert(
        rel_url,
        CacheEntry {
            mtime,
            metadata: metadata.clone(),
        },
    );

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

    Ok(metadata)
}

fn generate_articles_index(labels: &[String], path: &Path) -> Result<()> {
    let file = fs::File::create(path)?;
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

fn generate_tags_index(tags_map: &TagsMap, path: &Path) -> Result<()> {
    let file = fs::File::create(path)?;
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
    for tag in tags_map.keys() {
        writer.write_all(br#"<li><a href=""#)?;
        writer.write_all(slugify(tag).as_bytes())?;
        writer.write_all(br#".html">"#)?;
        writer.write_all(html_escape(tag).as_bytes())?;
        writer.write_all(b"</a></li>\n")?;
    }
    writer.write_all(b"</ul>\n")?;

    writer.write_all(FOOTER.as_bytes())?;
    writer.flush()?;

    Ok(())
}

fn generate_tag_pages(
    labels: Vec<String>,
    tags_map: TagsMap,
    output_dir: &Path,
) -> Result<Vec<String>> {
    tags_map
        .into_iter()
        .map(|(tag, (indices, src_modified))| {
            let slug = slugify(&tag);
            let escaped = html_escape(&tag);
            let tag_url = format!("{slug}.html");
            let dst_path = output_dir.join(&tag_url);

            if dst_path.try_exists()? && dst_path.metadata()?.modified()? > src_modified {
                return Ok(tag_url);
            }

            let file = fs::File::create(&dst_path)?;
            let mut writer = BufWriter::new(file);

            let header_str = generate_header(
                &escaped,
                &format!("Articles tagged {escaped}"),
                &format!("blog, blogpost, {escaped}"),
                &tag_url,
            );

            writer.write_all(header_str.as_bytes())?;
            writer.write_all(b"<h1>")?;
            writer.write_all(escaped.as_bytes())?;
            writer.write_all(b"</h1><hr>")?;

            writer.write_all(b"<ul>\n")?;
            for idx in indices {
                writer.write_all(b"<li>")?;
                writer.write_all(labels[idx].as_bytes())?;
                writer.write_all(b"</li>\n")?;
            }
            writer.write_all(b"</ul>\n")?;

            writer.write_all(FOOTER.as_bytes())?;
            writer.flush()?;

            Ok(tag_url)
        })
        .collect()
}

fn cleanup_stale(output_dir: &Path, expected: HashSet<String>) -> Result<()> {
    for entry in walkdir::WalkDir::new(output_dir).into_iter().flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str());
        if ext == Some("html") {
            let rel = path
                .strip_prefix(output_dir)?
                .to_str()
                .ok_or_else(|| anyhow!("Path not UTF8"))?;
            if !expected.contains(rel) {
                fs::remove_file(path)?
            }
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
