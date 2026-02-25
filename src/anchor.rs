use crate::syntex::{highlight_code, latex_to_mathml};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Tag, TagEnd};
use pulldown_latex::{Storage, config::DisplayMode};

#[derive(Default)]
pub(crate) enum HeadingCapture<'a> {
    Capturing(Vec<Event<'a>>, HeadingLevel, Option<CowStr<'a>>),
    Emitting(Vec<Event<'a>>, HeadingLevel),
    #[default]
    Inactive,
}

pub(crate) struct AnchorIterator<'a, I: Iterator<Item = Event<'a>>> {
    inner: I,
    storage: Storage,
    code: Option<CowStr<'a>>,
    heading: HeadingCapture<'a>,
}

impl<'a, I: Iterator<Item = Event<'a>>> AnchorIterator<'a, I> {
    pub(crate) fn new(inner: I) -> Self {
        Self {
            inner,
            storage: Storage::new(),
            code: None,
            heading: HeadingCapture::Inactive,
        }
    }
}

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for AnchorIterator<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let HeadingCapture::Emitting(h_events, h_level) = &mut self.heading {
            Some(match h_events.pop() {
                Some(Event::InlineMath(ref latex)) => Event::InlineHtml(CowStr::from(
                    latex_to_mathml(latex, &mut self.storage, DisplayMode::Inline).ok()?,
                )),

                Some(event) => event,

                None => {
                    let event = Event::Html(CowStr::from(format!("</a></{}>", h_level)));
                    self.heading = HeadingCapture::Inactive;
                    event
                }
            })
        } else {
            match self.inner.next()? {
                // -- Code --
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

                // -- Math
                Event::DisplayMath(latex) => Some(Event::Html(CowStr::from(
                    latex_to_mathml(&latex, &mut self.storage, DisplayMode::Block).ok()?,
                ))),

                Event::InlineMath(latex) if let HeadingCapture::Inactive = self.heading => {
                    Some(Event::InlineHtml(CowStr::from(
                        latex_to_mathml(&latex, &mut self.storage, DisplayMode::Inline).ok()?,
                    )))
                }

                // -- Anchor
                Event::Start(Tag::Heading { level, id, .. }) => {
                    self.heading = HeadingCapture::Capturing(Vec::with_capacity(1), level, id);
                    self.next()
                }

                Event::End(TagEnd::Heading(_level))
                    if let HeadingCapture::Capturing(mut h_events, h_level, id) =
                        std::mem::take(&mut self.heading) =>
                {
                    let id: String = id
                        .map(|s| s.into_string())
                        .unwrap_or_else(|| slugify(&h_events));

                    h_events.reverse();
                    self.heading = HeadingCapture::Emitting(h_events, h_level);

                    Some(Event::Html(CowStr::from(format!(
                        "<{} id=\"{}\"><a href=\"#{}\">",
                        h_level, id, id
                    ))))
                }

                // -- Table of Contents
                Event::Html(CowStr::Borrowed("<!--toc:start-->\n")) => Some(Event::Html(
                    CowStr::Borrowed("<details><summary>Contents</summary>"),
                )),

                Event::Html(CowStr::Borrowed("<!--toc:end-->\n")) => {
                    Some(Event::Html(CowStr::Borrowed("</details>")))
                }

                event if let HeadingCapture::Capturing(ref mut h_events, ..) = self.heading => {
                    h_events.push(event);
                    self.next()
                }

                event => Some(event),
            }
        }
    }
}

// Slugger

fn slugify(h_events: &[Event]) -> String {
    let mut last_dash = true;

    h_events
        .iter()
        .filter_map(|event| match event {
            Event::Text(s) | Event::Code(s) | Event::InlineMath(s) | Event::InlineHtml(s) => Some(
                s.chars()
                    .filter_map(|c| {
                        if c.is_alphanumeric() {
                            last_dash = false;
                            Some(c.to_ascii_lowercase())
                        } else if (c.is_whitespace() || c == '-') && !last_dash {
                            last_dash = true;
                            Some('-')
                        } else {
                            None
                        }
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<String>()
}
