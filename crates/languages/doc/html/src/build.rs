use crate::*;
use alloc::{string::String, vec::Vec};
use nepl3_core::budget::{Budget, Resource};
use nepl3_doc_core::model::*;
use nepl3_markup::html::*;

mod block;
mod inline;
const CLASSES: &[&str] = &[
    "nepl-doc",
    "nepl-paragraph",
    "nepl-ruby",
    "nepl-anno",
    "nepl-base",
    "nepl-reading",
    "nepl-notes",
    "nepl-note",
    "nepl-parallel-rows",
    "nepl-parallel-columns",
    "nepl-parallel-single",
    "nepl-align-left",
    "nepl-align-center",
    "nepl-align-right",
    "nepl-checkbox",
];
pub(super) fn copy(s: &str, b: &mut Budget) -> Result<String, StopReason> {
    b.charge(Resource::Work, s.len() as u64)?;
    b.charge(Resource::AllocationUnits, s.len() as u64)?;
    Ok(s.into())
}
pub(crate) fn push<T>(v: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<T>() as u64,
    )?;
    v.push(item);
    Ok(())
}
pub(super) fn hex_id(s: &str, b: &mut Budget) -> Result<String, StopReason> {
    let len = (s.len() as u64)
        .checked_mul(2)
        .and_then(|n| n.checked_add(2))
        .filter(|n| *n <= isize::MAX as u64)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::Work, len)?;
    b.charge(Resource::AllocationUnits, len)?;
    let mut out = String::with_capacity(len as usize);
    out.push_str("n-");
    for byte in s.bytes() {
        out.push(b"0123456789abcdef"[(byte >> 4) as usize] as char);
        out.push(b"0123456789abcdef"[(byte & 15) as usize] as char);
    }
    Ok(out)
}
#[derive(Clone, Copy)]
struct Job {
    node: u64,
    parent: u64,
    level: u64,
}
struct Builder<'a, 'b> {
    prepared: &'a crate::prepare::PreparedRendering<'a>,
    links: &'a [(u64, HtmlHref)],
    b: &'b mut Budget,
    nodes: Vec<HtmlNode>,
    depths: Vec<u64>,
    origins: Vec<ElementOrigin>,
    jobs: Vec<Job>,
}
impl Builder<'_, '_> {
    fn attr(&mut self, node: u64, attr: HtmlAttribute) -> Result<(), RenderError> {
        let Some(HtmlNode::Element { attributes, .. }) = self.nodes.get_mut(node as usize) else {
            return Err(RenderError::InternalShape);
        };
        push(attributes, attr, self.b)?;
        Ok(())
    }
    fn class(&mut self, node: u64, class: &str) -> Result<(), RenderError> {
        let value = copy(class, self.b)?;
        let mut values = Vec::new();
        push(&mut values, value, self.b)?;
        self.attr(node, HtmlAttribute::Class { values })
    }
    fn add(
        &mut self,
        parent: Option<u64>,
        cause: u64,
        value: HtmlNode,
    ) -> Result<u64, RenderError> {
        self.b.charge(Resource::Work, 1)?;
        let depth = match parent {
            Some(p) => self.depths[p as usize] + 1,
            None => 1,
        };
        // This HTML backend's supported output envelope; semantic input stays intact.
        if depth > 256 {
            return Err(RenderError::OutputDepth { node: cause });
        }
        self.b.observe_depth(depth)?;
        self.b.charge(Resource::Nodes, 1)?;
        let id = self.nodes.len() as u64;
        push(&mut self.nodes, value, self.b)?;
        push(&mut self.depths, depth, self.b)?;
        push(
            &mut self.origins,
            ElementOrigin {
                element: id,
                node: cause,
            },
            self.b,
        )?;
        if let Some(p) = parent {
            let HtmlNode::Element { children, .. } = &mut self.nodes[p as usize] else {
                return Err(RenderError::InternalShape);
            };
            push(children, id, self.b)?;
        }
        Ok(id)
    }
    fn element(
        &mut self,
        parent: Option<u64>,
        cause: u64,
        tag: HtmlTag,
    ) -> Result<u64, RenderError> {
        self.add(
            parent,
            cause,
            HtmlNode::Element {
                tag,
                attributes: Vec::new(),
                children: Vec::new(),
            },
        )
    }
    fn text(&mut self, parent: u64, cause: u64, text: &str) -> Result<(), RenderError> {
        let text = copy(text, self.b)?;
        self.add(Some(parent), cause, HtmlNode::Text { text })?;
        Ok(())
    }
    fn job(&mut self, node: u64, parent: u64, level: u64) -> Result<(), RenderError> {
        push(
            &mut self.jobs,
            Job {
                node,
                parent,
                level,
            },
            self.b,
        )?;
        Ok(())
    }
    fn heading(
        &mut self,
        parent: u64,
        cause: u64,
        title: u64,
        level: u64,
    ) -> Result<(), RenderError> {
        let tag = match level {
            1 => HtmlTag::H1,
            2 => HtmlTag::H2,
            3 => HtmlTag::H3,
            4 => HtmlTag::H4,
            5 => HtmlTag::H5,
            6 => HtmlTag::H6,
            _ => HtmlTag::Div,
        };
        let h = self.element(Some(parent), cause, tag)?;
        if level > 6 {
            self.attr(
                h,
                HtmlAttribute::Role {
                    value: HtmlRole::Heading,
                },
            )?;
            self.attr(h, HtmlAttribute::AriaLevel { value: level })?;
        }
        self.job(title, h, level)
    }
}
/// Convert a prepared Article with no external requirements. All generated
/// markup passes the common validator; this helper returns a fragment, not a
/// standalone document with assets or a saved SourceSnapshot.
pub fn render(
    prepared: &PreparedLocalArticle<'_>,
    budget: &mut Budget,
) -> Result<RenderedFragment, RenderError> {
    render_prepared(&prepared.0, &[], budget)
}
pub(crate) fn render_prepared(
    prepared: &crate::prepare::PreparedRendering<'_>,
    links: &[(u64, HtmlHref)],
    budget: &mut Budget,
) -> Result<RenderedFragment, RenderError> {
    budget.poll()?;
    let DocRoot::Article(root) = prepared.document.value.root else {
        return Err(RenderError::InternalShape);
    };
    let mut w = Builder {
        prepared,
        links,
        b: budget,
        nodes: Vec::new(),
        depths: Vec::new(),
        origins: Vec::new(),
        jobs: Vec::new(),
    };
    let article = w.element(None, root.0, HtmlTag::Article)?;
    w.class(article, "nepl-doc")?;
    let DocKind::Article {
        language,
        title,
        body,
    } = &prepared.document.value.nodes[root.0 as usize].kind
    else {
        return Err(RenderError::InternalShape);
    };
    let lang = copy(language, w.b)?;
    w.attr(article, HtmlAttribute::Lang { value: lang })?;
    w.job(body.0, article, 1)?;
    w.heading(article, root.0, title.0, 1)?;
    while let Some(job) = w.jobs.pop() {
        w.b.charge(Resource::Work, 1)?;
        let kind = &prepared.document.value.nodes[job.node as usize].kind;
        if !w.block(job, kind)? {
            w.inline(job, kind)?;
        }
    }
    let mut classes = Vec::new();
    for class in CLASSES {
        let value = copy(class, w.b)?;
        push(&mut classes, value, w.b)?;
    }
    let markup = HtmlRequest {
        fragment: HtmlFragment {
            root: article,
            nodes: w.nodes,
        },
        slot: HtmlSlot::Block,
        policy: HtmlPolicy { classes },
    };
    validate(&markup.fragment, markup.slot, &markup.policy, w.b)?;
    // Bind even visually identical outputs to their actual generation options.
    let parallel = match &prepared.options.parallel {
        ParallelMode::Rows => ParallelMode::Rows,
        ParallelMode::Columns => ParallelMode::Columns,
        ParallelMode::Single {
            language,
            fallbacks,
        } => {
            let language = copy(language, w.b)?;
            let mut values = Vec::new();
            for value in fallbacks {
                let value = copy(value, w.b)?;
                push(&mut values, value, w.b)?;
            }
            ParallelMode::Single {
                language,
                fallbacks: values,
            }
        }
    };
    Ok(RenderedFragment {
        document_digest: prepared.identity,
        options: RenderOptions { parallel },
        markup,
        origins: w.origins,
    })
}
