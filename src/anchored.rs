use crate::syntex::{highlight_code, latex_to_mathml};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Tag, TagEnd};
use pulldown_latex::{Storage, config::DisplayMode};

pub(crate) struct AnchoredIterator<'a, I: Iterator<Item = Event<'a>>> {
    inner: I,
    storage: Storage,
    code: Option<CowStr<'a>>,
    footnote: Option<CowStr<'a>>,
    heading: Head<'a>,
}

#[derive(Default)]
pub(crate) enum Head<'a> {
    #[default]
    Inactive,
    Capturing(Vec<Event<'a>>, HeadingLevel, Option<CowStr<'a>>),
    Emitting(Vec<Event<'a>>, HeadingLevel),
}

impl<'a, I: Iterator<Item = Event<'a>>> AnchoredIterator<'a, I> {
    pub(crate) fn new(inner: I) -> Self {
        Self {
            inner,
            storage: Storage::new(),
            code: None,
            footnote: None,
            heading: Head::Inactive,
        }
    }
}

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for AnchoredIterator<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Head::Emitting(h_events, h_level) = &mut self.heading {
            Some(match h_events.pop() {
                Some(Event::InlineMath(ref latex)) => Event::InlineHtml(CowStr::from(
                    latex_to_mathml(latex, &mut self.storage, DisplayMode::Inline).unwrap(),
                )),

                Some(event) => event,

                None => {
                    let event = Event::Html(CowStr::from(format!("</a></{h_level}>")));
                    self.heading = Head::Inactive;
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

                Event::Text(code) if let Some(lang) = &self.code => Some(Event::Html(
                    CowStr::from(highlight_code(&code, lang).unwrap()),
                )),

                Event::End(TagEnd::CodeBlock) if self.code.is_some() => {
                    self.code = None;
                    self.next()
                }

                // -- Math --
                Event::DisplayMath(latex) => Some(Event::Html(CowStr::from(
                    latex_to_mathml(&latex, &mut self.storage, DisplayMode::Block).unwrap(),
                ))),

                Event::InlineMath(latex) if let Head::Inactive = self.heading => {
                    Some(Event::InlineHtml(CowStr::from(
                        latex_to_mathml(&latex, &mut self.storage, DisplayMode::Inline).unwrap(),
                    )))
                }

                // -- Footnotes --
                Event::FootnoteReference(name) => Some(Event::InlineHtml(CowStr::from(format!(
                    "<sup class=\"footnote-reference\" id=\"fr-{name}\"><a href=\"#{name}\">{name}</a></sup>"
                )))),

                Event::Start(Tag::FootnoteDefinition(name)) => {
                    self.footnote = Some(name.clone());
                    Some(Event::Start(Tag::FootnoteDefinition(name)))
                }

                Event::End(TagEnd::FootnoteDefinition) if let Some(name) = self.footnote.take() => {
                    Some(Event::Html(CowStr::from(format!(
                        " <a href=\"#fr-{name}\">↩</a></div>\n"
                    ))))
                }

                // -- Table of Contents
                Event::Html(CowStr::Borrowed("<!--toc:start-->\n")) => Some(Event::Html(
                    CowStr::Borrowed("<details class=\"toc\"><summary>Contents</summary>"),
                )),

                Event::Html(CowStr::Borrowed("<!--toc:end-->\n")) => {
                    Some(Event::Html(CowStr::Borrowed("</details>")))
                }

                // -- Anchors
                Event::Start(Tag::Heading { level, id, .. }) => {
                    self.heading = Head::Capturing(Vec::with_capacity(1), level, id);
                    self.next()
                }

                Event::End(TagEnd::Heading(_level))
                    if let Head::Capturing(mut h_events, h_level, id) =
                        std::mem::take(&mut self.heading) =>
                {
                    let id: String = id
                        .map(|s| s.into_string())
                        .unwrap_or_else(|| slugify_h(&h_events));

                    h_events.reverse();
                    self.heading = Head::Emitting(h_events, h_level);

                    Some(Event::Html(CowStr::from(format!(
                        "<{h_level} id=\"{id}\"><a href=\"#{id}\">"
                    ))))
                }

                event if let Head::Capturing(ref mut h_events, ..) = self.heading => {
                    h_events.push(event);
                    self.next()
                }

                event => Some(event),
            }
        }
    }
}

// Slugger

fn slugify_h(h_events: &[Event]) -> String {
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
