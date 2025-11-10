use crate::{
    syntex::{highlight_code, latex_to_mathml},
    types::Levels,
    utils::Slugger,
};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Tag, TagEnd};
use pulldown_latex::{Storage, config::DisplayMode};

#[derive(Default)]
pub(crate) enum HeadingT<'a> {
    Capturing(
        Vec<Event<'a>>,
        HeadingLevel,
        Option<CowStr<'a>>,
        Vec<CowStr<'a>>,
        Vec<(CowStr<'a>, Option<CowStr<'a>>)>,
    ),
    Emitting(Vec<Event<'a>>, HeadingLevel),
    #[default]
    Inactive,
}

pub(crate) struct TocIterator<'a, I: Iterator<Item = Event<'a>>> {
    inner: I,
    storage: Storage,
    syntax_tag: Option<CowStr<'a>>,
    captive_string: Option<String>,
    table: &'a mut String,
    levels: Levels,
    heading: HeadingT<'a>,
    slugger: Slugger,
}

impl<'a, I: Iterator<Item = Event<'a>>> TocIterator<'a, I> {
    pub(crate) fn new(inner: I, h: Levels, table: &'a mut String) -> Self {
        Self {
            inner,
            storage: Storage::new(),
            syntax_tag: None,
            captive_string: None,
            table,
            levels: h,
            heading: HeadingT::Inactive,
            slugger: Slugger::new(),
        }
    }
}

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for TocIterator<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let HeadingT::Emitting(ref mut h_events, level) = self.heading {
            return Some(h_events.pop().unwrap_or_else(|| {
                self.heading = HeadingT::Inactive;
                Event::End(TagEnd::Heading(level))
            }));
        };

        let event = self.inner.next()?;

        match event {
            Event::Text(ref t) if let Some(s) = self.captive_string.as_mut() => {
                s.push_str(t);
                self.next()
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                self.captive_string = Some(String::new());
                self.syntax_tag = match kind {
                    CodeBlockKind::Fenced(lang) => Some(lang),
                    CodeBlockKind::Indented => None,
                };
                self.next()
            }

            Event::End(TagEnd::CodeBlock)
                if let Some(code) = self.captive_string.take()
                    && let Some(lang) = self.syntax_tag.take() =>
            {
                let highlighted = highlight_code(&code, &lang).ok()?;
                Some(Event::Html(CowStr::from(highlighted)))
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
                    HeadingT::Capturing(Vec::with_capacity(1), level, id, classes, attrs);
                self.next()
            }

            Event::End(TagEnd::Heading(_))
                if let HeadingT::Capturing(mut h_events, level, id, classes, attrs) =
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

                self.heading = HeadingT::Emitting(h_events, level);

                Some(Event::Start(Tag::Heading {
                    level,
                    id: Some(id),
                    classes,
                    attrs,
                }))
            }

            _ if let HeadingT::Capturing(ref mut h_events, ..) = self.heading => {
                h_events.push(event);
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
