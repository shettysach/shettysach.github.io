use chrono::{DateTime, Utc};
use pulldown_cmark::{CowStr, Event, HeadingLevel};

pub(crate) struct Frontmatter {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) tags: Option<Vec<String>>,
}

pub(crate) struct Entries {
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) datetime: DateTime<Utc>,
    pub(crate) url: String,
}

pub(crate) struct CaptiveHeading<'a> {
    pub(crate) level: HeadingLevel,
    pub(crate) id: Option<CowStr<'a>>,
    pub(crate) classes: Vec<CowStr<'a>>,
    pub(crate) attrs: Vec<(CowStr<'a>, Option<CowStr<'a>>)>,
    pub(crate) h_events: Vec<Event<'a>>,
}

pub(crate) struct EmittingHeading<'a> {
    pub(crate) level: HeadingLevel,
    pub(crate) h_events: Vec<Event<'a>>,
}
