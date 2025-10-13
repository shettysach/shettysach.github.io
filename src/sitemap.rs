use crate::types::Entries;
use anyhow::Result;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::writer::Writer;
use std::{fs, path::Path};

pub(crate) fn generate_sitemap(entries: &[Entries], path: &Path) -> Result<()> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    let mut urlset_start = BytesStart::new("urlset");
    urlset_start.push_attribute(("xmlns", "http://www.sitemaps.org/schemas/sitemap/0.9"));
    writer.write_event(Event::Start(urlset_start))?;

    let static_pages = [
        ("index.html", "1.0"),
        ("articles.html", "0.9"),
        ("tags.html", "0.8"),
    ];
    for (page, pri) in static_pages {
        writer.write_event(Event::Start(BytesStart::new("url")))?;

        // loc
        writer.write_event(Event::Start(BytesStart::new("loc")))?;
        let full_url = format!("https://shettysach.github.io/{}", page);
        writer.write_event(Event::Text(BytesText::new(&full_url)))?;
        writer.write_event(Event::End(BytesEnd::new("loc")))?;

        // lastmod
        writer.write_event(Event::Start(BytesStart::new("lastmod")))?;
        writer.write_event(Event::Text(BytesText::new("2025-10-13T00:00:00Z")))?;
        writer.write_event(Event::End(BytesEnd::new("lastmod")))?;

        // priority
        writer.write_event(Event::Start(BytesStart::new("priority")))?;
        writer.write_event(Event::Text(BytesText::new(pri)))?;
        writer.write_event(Event::End(BytesEnd::new("priority")))?;

        writer.write_event(Event::End(BytesEnd::new("url")))?;
    }

    for Entries { url, datetime, .. } in entries {
        writer.write_event(Event::Start(BytesStart::new("url")))?;

        // loc
        writer.write_event(Event::Start(BytesStart::new("loc")))?;
        let full_url = format!("https://shettysach.github.io/{}", url);
        writer.write_event(Event::Text(BytesText::new(&full_url)))?;
        writer.write_event(Event::End(BytesEnd::new("loc")))?;

        // lastmod
        writer.write_event(Event::Start(BytesStart::new("lastmod")))?;
        writer.write_event(Event::Text(BytesText::new(&datetime.to_rfc3339())))?;
        writer.write_event(Event::End(BytesEnd::new("lastmod")))?;

        // priority
        writer.write_event(Event::Start(BytesStart::new("priority")))?;
        writer.write_event(Event::Text(BytesText::new("0.8")))?;
        writer.write_event(Event::End(BytesEnd::new("priority")))?;

        writer.write_event(Event::End(BytesEnd::new("url")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("urlset")))?;

    let xml = writer.into_inner();
    fs::write(path, xml)?;

    Ok(())
}
