use chrono::{DateTime, Utc};

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
