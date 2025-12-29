use crate::syntex::{highlight_code, latex_to_mathml};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Tag, TagEnd};
use pulldown_latex::{Storage, config::DisplayMode};

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

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for &mut AnchorIterator<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let HeadingCapture::Emitting(ref mut h_events, level) = self.heading {
            let w = match h_events.pop() {
                Some(Event::InlineMath(ref latex)) => {
                    let mathml =
                        latex_to_mathml(latex, &mut self.storage, DisplayMode::Inline).ok()?;
                    Event::InlineHtml(CowStr::from(mathml))
                }
                Some(event) => event,
                None => {
                    self.heading = HeadingCapture::Inactive;
                    Event::End(TagEnd::Heading(level))
                }
            };
            return Some(w);
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

            event @ Event::InlineMath(_)
                if let HeadingCapture::Capturing(ref mut head) = self.heading =>
            {
                head.0.push(event);
                self.next()
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
            }) => {
                self.heading =
                    HeadingCapture::Capturing((Vec::with_capacity(1), level, id, classes, attrs));
                self.next()
            }

            Event::End(TagEnd::Heading(_level))
                if let HeadingCapture::Capturing((mut h_events, level, id, classes, attrs)) =
                    std::mem::take(&mut self.heading) =>
            {
                let id = id.or_else(|| {
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
                    Some(CowStr::Boxed(slugify(&header_text).into()))
                });

                h_events.reverse();

                self.heading = HeadingCapture::Emitting(h_events, level);

                Some(Event::Start(Tag::Heading {
                    level,
                    id,
                    classes,
                    attrs,
                }))
            }

            Event::InlineHtml(CowStr::Borrowed("<!--toc:start-->\n"))
            | Event::Html(CowStr::Borrowed("<!--toc:start-->\n")) => Some(Event::Html(
                CowStr::Borrowed("<details><summary>Contents</summary>"),
            )),

            Event::InlineHtml(CowStr::Borrowed("<!--toc:end-->\n"))
            | Event::Html(CowStr::Borrowed("<!--toc:end-->\n")) => {
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

// Slugger

fn slugify(s: &str) -> String {
    let mut last_dash = true;
    let iter = s.chars();

    iter.filter_map(|c| {
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
    .collect()
}
