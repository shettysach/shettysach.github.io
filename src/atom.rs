use std::{fs, path::Path};

use anyhow::Result;
use chrono::{DateTime, Utc};
use xxhash_rust::xxh32::Xxh32;

pub struct FeedEntry {
    pub title: String,
    pub link: String,
    pub updated: DateTime<Utc>,
    pub summary: Option<String>,
}

fn escape_xml(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
}

pub fn generate_atom_feed(
    entries: Vec<FeedEntry>,
    feed_title: &str,
    feed_link: &str,
    output_path: &Path,
) -> Result<()> {
    let mut hasher = Xxh32::new(0);

    hasher.update(feed_title.as_bytes());
    hasher.update(feed_link.as_bytes());

    for entry in &entries {
        hasher.update(entry.title.as_bytes());
        hasher.update(entry.link.as_bytes());
        hasher.update(&entry.updated.timestamp().to_le_bytes());
        if let Some(ref summary) = entry.summary {
            hasher.update(summary.as_bytes());
        }
    }
    let input_hash = hasher.digest();

    let cache_dir = output_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(".cache");
    fs::create_dir_all(&cache_dir)?;
    let cache_hash_path = cache_dir.join("atom.hash");

    // Check if cache is valid
    if cache_hash_path.exists() {
        if let Ok(cached_hash_bytes) = fs::read(&cache_hash_path) {
            if cached_hash_bytes.len() == 4 {
                let cached_hash = u32::from_le_bytes(cached_hash_bytes.try_into().unwrap());
                if cached_hash == input_hash {
                    // Cache hit, output is already up to date
                    return Ok(());
                }
            }
        }
    }

    let mut xml_content = String::new();

    // XML declaration
    xml_content.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");

    // Start feed element
    xml_content.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");

    // Feed title
    xml_content.push_str(&format!("  <title>{}</title>\n", escape_xml(feed_title)));

    // Feed link
    xml_content.push_str(&format!(
        "  <link href=\"{}\" rel=\"alternate\" type=\"text/html\"/>\n",
        escape_xml(feed_link)
    ));

    // Feed id
    xml_content.push_str(&format!("  <id>{}</id>\n", escape_xml(feed_link)));

    // Feed updated
    let latest_updated = entries
        .iter()
        .map(|e| e.updated)
        .max()
        .unwrap_or_else(Utc::now);
    xml_content.push_str(&format!(
        "  <updated>{}</updated>\n",
        latest_updated.to_rfc3339()
    ));

    // Entries
    for entry in entries {
        xml_content.push_str("  <entry>\n");

        // Entry title
        xml_content.push_str(&format!(
            "    <title>{}</title>\n",
            escape_xml(&entry.title)
        ));

        // Entry link
        xml_content.push_str(&format!(
            "    <link href=\"{}\" rel=\"alternate\" type=\"text/html\"/>\n",
            escape_xml(&entry.link)
        ));

        // Entry id
        xml_content.push_str(&format!("    <id>{}</id>\n", escape_xml(&entry.link)));

        // Entry updated
        xml_content.push_str(&format!(
            "    <updated>{}</updated>\n",
            entry.updated.to_rfc3339()
        ));

        // Entry summary (optional)
        if let Some(summary) = entry.summary {
            xml_content.push_str(&format!(
                "    <summary>{}</summary>\n",
                escape_xml(&summary)
            ));
        }

        xml_content.push_str("  </entry>\n");
    }

    // End feed
    xml_content.push_str("</feed>\n");

    fs::write(output_path, &xml_content)?;

    // Write hash to cache
    fs::write(&cache_hash_path, input_hash.to_le_bytes())?;

    Ok(())
}
