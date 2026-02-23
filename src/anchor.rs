use crate::syntex::{highlight_code, latex_to_mathml};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Tag, TagEnd};
use pulldown_latex::{Storage, config::DisplayMode};
use std::mem::take;

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
    Emitting(Emit<'a>),
    #[default]
    Inactive,
}

pub(crate) enum Emit<'a> {
    AnchorOpen(Vec<Event<'a>>, HeadingLevel, String),
    Content(Vec<Event<'a>>, HeadingLevel),
    AnchorClose(HeadingLevel),
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
        if let HeadingCapture::Emitting(_) = &mut self.heading
            && let HeadingCapture::Emitting(emit) = take(&mut self.heading)
        {
            Some(match emit {
                Emit::AnchorOpen(mut events, level, id) => {
                    events.reverse();
                    self.heading = HeadingCapture::Emitting(Emit::Content(events, level));
                    Event::Html(format!("<a href=\"#{}\">", id).into())
                }
                Emit::Content(mut events, level) => match events.pop() {
                    Some(Event::InlineMath(ref latex)) => {
                        let mathml =
                            latex_to_mathml(latex, &mut self.storage, DisplayMode::Inline).ok()?;
                        self.heading = HeadingCapture::Emitting(Emit::Content(events, level));
                        Event::InlineHtml(CowStr::from(mathml))
                    }
                    Some(event) => {
                        self.heading = HeadingCapture::Emitting(Emit::Content(events, level));
                        event
                    }
                    None => {
                        self.heading = HeadingCapture::Emitting(Emit::AnchorClose(level));
                        Event::End(TagEnd::Link)
                    }
                },
                Emit::AnchorClose(level) => Event::End(TagEnd::Heading(level)),
            })
        } else {
            match self.inner.next()? {
                // -- Code --
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                    self.code = Some(lang);
                    Some(Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)))
                }

                Event::Text(code) if let Some(lang) = take(&mut self.code) => Some(Event::Html(
                    CowStr::from(highlight_code(&code, &lang).ok()?),
                )),

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
                Event::Start(Tag::Heading {
                    level,
                    id,
                    classes,
                    attrs,
                }) => {
                    self.heading = HeadingCapture::Capturing((
                        Vec::with_capacity(1),
                        level,
                        id,
                        classes,
                        attrs,
                    ));
                    self.next()
                }

                Event::End(TagEnd::Heading(_level))
                    if let HeadingCapture::Capturing((h_events, level, id, classes, attrs)) =
                        take(&mut self.heading) =>
                {
                    let id: String = id
                        .map(|s| s.into_string())
                        .unwrap_or_else(|| slugify(&h_events));

                    self.heading =
                        HeadingCapture::Emitting(Emit::AnchorOpen(h_events, level, id.clone()));

                    Some(Event::Start(Tag::Heading {
                        level,
                        id: Some(id.into()),
                        classes,
                        attrs,
                    }))
                }

                Event::Html(CowStr::Borrowed("<!--toc:start-->\n")) => Some(Event::Html(
                    CowStr::Borrowed("<details><summary>Contents</summary>"),
                )),

                Event::Html(CowStr::Borrowed("<!--toc:end-->\n")) => {
                    Some(Event::Html(CowStr::Borrowed("</details>")))
                }

                event if let HeadingCapture::Capturing(ref mut head) = self.heading => {
                    head.0.push(event);
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
