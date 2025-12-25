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
    headings: Vec<(HeadingLevel, String, String)>,
    heading: HeadingCapture<'a>,
    slugger: Slugger<32>,
    levels: Levels,
}

impl<'a, I: Iterator<Item = Event<'a>>> TocIterator<'a, I> {
    pub(crate) fn new(inner: I, levels: Levels) -> Self {
        Self {
            inner,
            levels,
            headings: Vec::new(),
            storage: Storage::new(),
            code: None,
            heading: HeadingCapture::Inactive,
            slugger: Slugger::new(),
        }
    }

    pub(crate) fn toc(self) -> String {
        if self.headings.is_empty() {
            return String::new();
        }

        let est_size = self.headings.len() * 50;
        let mut html = String::with_capacity(est_size);

        html.push_str("<ul>");
        let mut current_level = 1;

        for (i, (level, text, id)) in self.headings.iter().enumerate() {
            let level_num = Levels::level_to_u8(*level) as usize;

            // Close previous item (except for first iteration)
            if i > 0 {
                html.push_str("</li>");
            }

            // Open nested lists if going deeper
            if level_num > current_level {
                for _ in current_level..level_num {
                    html.push_str("<ul>");
                }
            }
            // Close nested lists if going shallower
            else if level_num < current_level {
                for _ in level_num..current_level {
                    html.push_str("</ul></li>");
                }
            }

            current_level = level_num;

            // Write the list item (kept separate for clarity, but could be combined)
            html.push_str(&format!("<li><a href=\"#{}\">{}</a>", id, text));
        }

        // Close all remaining open lists
        for _ in 1..current_level {
            html.push_str("</ul></li>");
        }
        html.push_str("</li></ul>");

        html
    }
}

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for &mut TocIterator<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let HeadingCapture::Emitting(ref mut h_events, level) = self.heading {
            return Some(h_events.pop().unwrap_or_else(|| {
                self.heading = HeadingCapture::Inactive;
                Event::End(TagEnd::Heading(level))
            }));
        };

        match self.inner.next()? {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                self.code = Some(lang);
                Some(Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)))
            }

            Event::Text(code) if let Some(lang) = self.code.as_mut() => {
                let highlighted = highlight_code(&code, lang).ok()?;
                Some(Event::Html(CowStr::from(highlighted)))
            }

            event @ Event::End(TagEnd::CodeBlock) if self.code.is_some() => {
                self.code = None;
                Some(event)
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

                self.headings
                    .push((level, header_text, id.as_ref().to_string()));

                h_events.reverse();

                self.heading = HeadingCapture::Emitting(h_events, level);

                Some(Event::Start(Tag::Heading {
                    level,
                    id: Some(id),
                    classes,
                    attrs,
                }))
            }

            event if let HeadingCapture::Capturing(ref mut head) = self.heading => {
                head.0.push(event);
                self.next()
            }

            event => Some(event),
        }
    }
}

// Heading levels

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
