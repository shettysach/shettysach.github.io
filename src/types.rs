use chrono::{DateTime, Utc};
use pulldown_cmark::Event;

pub(crate) struct Frontmatter {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) tags: Option<Vec<String>>,
}

pub(crate) struct Article<'a> {
    pub(crate) frontmatter: Frontmatter,
    pub(crate) events: Vec<Event<'a>>,
    pub(crate) toc: Option<String>,
}

pub(crate) struct Entries {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) datetime: DateTime<Utc>,
    pub(crate) url: String,
}

