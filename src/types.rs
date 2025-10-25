use chrono::{DateTime, Utc};
use pulldown_cmark::HeadingLevel;

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

use std::num::NonZeroU8;

#[derive(Clone, Copy)]
pub(crate) struct Levels(NonZeroU8);

impl Levels {
    #[inline]
    pub(crate) fn new(min: u8, max: u8) -> Option<Self> {
        ((1..=6).contains(&min) && (min..=6).contains(&max))
            .then(|| unsafe { NonZeroU8::new_unchecked(min << 4 | max) })
            .map(Self)
    }

    #[inline]
    pub(crate) fn min_level(&self) -> u8 {
        self.0.get() >> 4
    }

    #[inline]
    pub(crate) fn max_level(&self) -> u8 {
        self.0.get() & 0x0F
    }

    #[inline]
    pub(crate) fn level_to_u8(level: HeadingLevel) -> u8 {
        match level {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
            HeadingLevel::H5 => 5,
            HeadingLevel::H6 => 6,
        }
    }

    #[inline]
    pub(crate) fn level_enabled(&self, level: HeadingLevel) -> bool {
        (self.min_level()..=self.max_level()).contains(&Self::level_to_u8(level))
    }
}
