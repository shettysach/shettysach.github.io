use anyhow::Result;
use chrono::{DateTime, Utc};
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::writer::Writer;
use std::fs;
use std::path::Path;

pub(crate) struct Atom {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) datetime: DateTime<Utc>,
    pub(crate) url: String,
}

pub(crate) fn generate_atom_feed(entries: Vec<Atom>, path: &Path) -> Result<()> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    let mut feed_start = BytesStart::new("feed");
    feed_start.push_attribute(("xmlns", "http://www.w3.org/2005/Atom"));
    writer.write_event(Event::Start(feed_start))?;

    // author
    writer.write_event(Event::Start(BytesStart::new("author")))?;

    writer.write_event(Event::Start(BytesStart::new("name")))?;
    writer.write_event(Event::Text(BytesText::new("Sachith Shetty")))?;
    writer.write_event(Event::End(BytesEnd::new("name")))?;

    writer.write_event(Event::Start(BytesStart::new("email")))?;
    writer.write_event(Event::Text(BytesText::new("shettysachith47@gmail.com")))?;
    writer.write_event(Event::End(BytesEnd::new("email")))?;

    writer.write_event(Event::End(BytesEnd::new("author")))?;

    // title
    writer.write_event(Event::Start(BytesStart::new("title")))?;
    writer.write_event(Event::Text(BytesText::new("Sachith's Blog")))?;
    writer.write_event(Event::End(BytesEnd::new("title")))?;

    // id
    writer.write_event(Event::Start(BytesStart::new("id")))?;
    writer.write_event(Event::Text(BytesText::new("https://shettysach.github.io/")))?;
    writer.write_event(Event::End(BytesEnd::new("id")))?;

    // link
    let mut link = BytesStart::new("link");
    link.push_attribute(("href", "https://shettysach.github.io/atom.xml"));
    link.push_attribute(("rel", "self"));
    writer.write_event(Event::Empty(link))?;

    // updated - latest
    let now = Utc::now();
    let latest = entries.iter().map(|a| &a.datetime).max().unwrap_or(&now);
    writer.write_event(Event::Start(BytesStart::new("updated")))?;
    writer.write_event(Event::Text(BytesText::new(&latest.to_rfc3339())))?;
    writer.write_event(Event::End(BytesEnd::new("updated")))?;

    // entries
    for Atom {
        title,
        subtitle,
        datetime: date,
        url,
    } in entries
    {
        writer.write_event(Event::Start(BytesStart::new("entry")))?;

        // title
        writer.write_event(Event::Start(BytesStart::new("title")))?;
        writer.write_event(Event::Text(BytesText::new(&title)))?;
        writer.write_event(Event::End(BytesEnd::new("title")))?;

        // id - use url
        let url = format!("https://shettysach.github.io/{}", url);
        writer.write_event(Event::Start(BytesStart::new("id")))?;
        writer.write_event(Event::Text(BytesText::new(&url)))?;
        writer.write_event(Event::End(BytesEnd::new("id")))?;

        // updated
        writer.write_event(Event::Start(BytesStart::new("updated")))?;
        writer.write_event(Event::Text(BytesText::new(&date.to_rfc3339())))?;
        writer.write_event(Event::End(BytesEnd::new("updated")))?;

        // link
        let mut link = BytesStart::new("link");
        link.push_attribute(("href", url.as_str()));
        writer.write_event(Event::Empty(link))?;

        // summary
        if let Some(sub) = &subtitle {
            writer.write_event(Event::Start(BytesStart::new("summary")))?;
            writer.write_event(Event::Text(BytesText::new(sub)))?;
            writer.write_event(Event::End(BytesEnd::new("summary")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("entry")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("feed")))?;

    let xml = writer.into_inner();
    fs::write(path, xml)?;

    Ok(())
}

