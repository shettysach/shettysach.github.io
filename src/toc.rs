use crate::syntex::{highlight_code, latex_to_mathml};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Tag, TagEnd};
use pulldown_latex::{Storage, config::DisplayMode};
use xxhash_rust::xxh32::xxh32;

type HeadingType<'a> = (
    Vec<Event<'a>>,
    HeadingLevel,
    Option<CowStr<'a>>,
    Vec<CowStr<'a>>,
    Vec<(CowStr<'a>, Option<CowStr<'a>>)>,
);

#[derive(Default)]
pub(crate) enum HeadingCapture<'a> {
    Capturing(HeadingType<'a>),
    Emitting(Vec<Event<'a>>, HeadingLevel),
    #[default]
    Inactive,
}

pub(crate) struct TocIterator<'a, I: Iterator<Item = Event<'a>>> {
    inner: I,
    storage: Storage,
    code: Option<CowStr<'a>>,
    table: &'a mut String,
    heading: HeadingCapture<'a>,
    slugger: Slugger<32>,
    levels: Levels,
}

impl<'a, I: Iterator<Item = Event<'a>>> TocIterator<'a, I> {
    pub(crate) fn new(inner: I, levels: Levels, table: &'a mut String) -> Self {
        Self {
            inner,
            levels,
            table,
            storage: Storage::new(),
            code: None,
            heading: HeadingCapture::Inactive,
            slugger: Slugger::new(),
        }
    }
}

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for TocIterator<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let HeadingCapture::Emitting(ref mut h_events, level) = self.heading {
            return Some(h_events.pop().unwrap_or_else(|| {
                self.heading = HeadingCapture::Inactive;
                Event::End(TagEnd::Heading(level))
            }));
        };

        let event = self.inner.next()?;

        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                self.code = Some(lang);
                self.next()
            }

            Event::Text(code) if let Some(lang) = self.code.as_mut() => {
                let highlighted = highlight_code(&code, lang).ok()?;
                Some(Event::Html(CowStr::from(highlighted)))
            }

            Event::End(TagEnd::CodeBlock) if self.code.is_some() => {
                self.code = None;
                self.next()
            }

            Event::DisplayMath(latex) => {
                let mathml = latex_to_mathml(&latex, &mut self.storage, DisplayMode::Block).ok()?;
                Some(Event::Html(CowStr::from(mathml)))
            }

            Event::InlineMath(ref latex) => {
                let mathml = latex_to_mathml(latex, &mut self.storage, DisplayMode::Inline).ok()?;
                Some(Event::InlineHtml(CowStr::from(mathml)))
            }

            Event::Start(Tag::Heading {
                level,
                id,
                classes,
                attrs,
            }) if self.levels.level_enabled(level) => {
                self.heading =
                    HeadingCapture::Capturing((Vec::with_capacity(1), level, id, classes, attrs));
                self.next()
            }

            Event::End(TagEnd::Heading(_level))
                if let HeadingCapture::Capturing((mut h_events, level, id, classes, attrs)) =
                    std::mem::take(&mut self.heading) =>
            {
                let header_text = h_events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Text(s)
                        | Event::Code(s)
                        | Event::InlineMath(s)
                        | Event::InlineHtml(s) => Some(s.as_ref()),
                        _ => None,
                    })
                    .collect::<String>();

                let id = id.unwrap_or_else(|| CowStr::from(self.slugger.slug(&header_text)));

                self.table
                    .push_str(&table_bullet(level, self.levels, &header_text, &id));

                h_events.reverse();

                self.heading = HeadingCapture::Emitting(h_events, level);

                Some(Event::Start(Tag::Heading {
                    level,
                    id: Some(id),
                    classes,
                    attrs,
                }))
            }

            _ if let HeadingCapture::Capturing(ref mut hbox) = self.heading => {
                hbox.0.push(event);
                self.next()
            }

            _ => Some(event),
        }
    }
}

pub(crate) fn table_bullet(
    level: HeadingLevel,
    h: Levels,
    heading: &str,
    anchor_id: &str,
) -> String {
    let count = Levels::level_to_u8(level) - h.min_level();
    let indent = "  ".repeat(count as usize);
    format!("{indent}- [{heading}](#{anchor_id})\n")
}

// Headings

use std::num::NonZeroU8;

#[derive(Clone, Copy)]
pub(crate) struct Levels(NonZeroU8);

impl Levels {
    #[inline]
    pub(crate) fn new(min: u8, max: u8) -> Option<Self> {
        ((1..=6).contains(&min) && (min..=6).contains(&max))
            .then_some(min << 4 | max)
            .and_then(NonZeroU8::new)
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
        let level_num = Self::level_to_u8(level);
        (self.min_level()..=self.max_level()).contains(&level_num)
    }
}

// Slugger

pub struct Slugger<const N: usize> {
    counts: [u8; N],
}

impl<const N: usize> Slugger<N> {
    pub(crate) fn new() -> Self {
        Slugger { counts: [0; N] }
    }

    pub(crate) fn slug(&mut self, input: &str) -> String {
        let base = slugify(input);
        let hash = xxh32(base.as_bytes(), 0).to_le() as usize;
        let key = hash % N;

        let c = &mut self.counts[key];
        let slug = if *c != 0 {
            format!("{}_{}", base, *c)
        } else {
            base
        };

        *c = c.wrapping_add(1);
        slug
    }
}

impl<const N: usize> Default for Slugger<N> {
    fn default() -> Self {
        Self::new()
    }
}

fn slugify(s: &str) -> String {
    let mut last_dash = true;
    let iter = s.chars();

    iter.filter_map(|c| {
        if c.is_alphanumeric() {
            last_dash = false;
            Some(c.to_ascii_lowercase())
        } else if c.is_whitespace() && !last_dash {
            last_dash = true;
            Some('-')
        } else {
            None
        }
    })
    .collect()
}
