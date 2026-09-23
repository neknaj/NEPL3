use crate::*;
use alloc::{string::String, vec::Vec};
use nepl3_core::budget::{Budget, Resource};
use nepl3_doc_core::model::*;
use nepl3_markup::html::*;

mod block;
mod inline;
const CLASSES: &[&str] = &[
    "nepl-doc",
    "nepl-sentence",
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
    classes: Vec<String>,
    foreign: Vec<ForeignPlacement>,
}
impl Builder<'_, '_> {
    fn guest(&mut self, job: Job, embed: EmbedRef, markup: HtmlRequest) -> Result<(), RenderError> {
        let depth = self
            .b
            .current_depth()
            .saturating_add(self.depths[job.parent as usize]);
        self.b.with_depth_at_least::<_, RenderError>(depth, |b| {
            validate(&markup.fragment, HtmlSlot::Phrasing, &markup.policy, b)?;
            Ok(())
        })?;
        // Preserve the backend's output envelope across the guest boundary.
        // Validation above establishes references and acyclicity; this walk
        // measures each occurrence relative to its actual Doc parent.
        let mut pending = Vec::new();
        push(
            &mut pending,
            (markup.fragment.root, self.depths[job.parent as usize] + 1),
            self.b,
        )?;
        while let Some((index, depth)) = pending.pop() {
            self.b.charge(Resource::Work, 1)?;
            if depth > 256 {
                return Err(RenderError::OutputDepth { node: job.node });
            }
            match &markup.fragment.nodes[index as usize] {
                HtmlNode::Element { children, .. } | HtmlNode::MathElement { children, .. } => {
                    for child in children {
                        push(&mut pending, (*child, depth + 1), self.b)?;
                    }
                }
                HtmlNode::Text { .. } => {}
            }
        }
        let offset = self.nodes.len() as u64;
        let count = markup.fragment.nodes.len() as u64;
        let root = offset
            .checked_add(markup.fragment.root)
            .ok_or(RenderError::InternalShape)?;
        for mut node in markup.fragment.nodes {
            self.b.charge(Resource::Nodes, 1)?;
            match &mut node {
                HtmlNode::Element { children, .. } | HtmlNode::MathElement { children, .. } => {
                    for child in children {
                        self.b.charge(Resource::Work, 1)?;
                        *child = offset
                            .checked_add(*child)
                            .ok_or(RenderError::InternalShape)?;
                    }
                }
                HtmlNode::Text { .. } => {}
            }
            let element = self.nodes.len() as u64;
            push(&mut self.nodes, node, self.b)?;
            // Imported subtrees are complete; local jobs never attach to them.
            push(&mut self.depths, 0, self.b)?;
            push(
                &mut self.origins,
                ElementOrigin {
                    element,
                    node: job.node,
                },
                self.b,
            )?;
        }
        let Some(HtmlNode::Element { children, .. }) = self.nodes.get_mut(job.parent as usize)
        else {
            return Err(RenderError::InternalShape);
        };
        push(children, root, self.b)?;
        for class in markup.policy.classes {
            let mut present = false;
            for prior in &self.classes {
                self.b
                    .charge(Resource::Work, prior.len().min(class.len()) as u64 + 1)?;
                if prior == &class {
                    present = true;
                    break;
                }
            }
            if !present {
                push(&mut self.classes, class, self.b)?;
            }
        }
        push(
            &mut self.foreign,
            ForeignPlacement {
                embed,
                first_element: offset,
                elements: count,
            },
            self.b,
        )?;
        Ok(())
    }
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
/// Render a prepared standalone Sentence as checked phrasing markup. Ruby and
/// Anno use the same tree and stylesheet contract as Article rendering.
pub fn render_sentence(
    prepared: &PreparedLocalSentence<'_>,
    budget: &mut Budget,
) -> Result<RenderedFragment, RenderError> {
    render_prepared(&prepared.0, &[], budget)
}
/// Render a prepared Inline fragment as checked phrasing markup, preserving
/// each emitted element's Doc node owner.
pub fn render_inline(
    prepared: &PreparedLocalInline<'_>,
    budget: &mut Budget,
) -> Result<RenderedFragment, RenderError> {
    render_prepared(&prepared.0, &[], budget)
}
pub(crate) fn render_prepared(
    prepared: &crate::prepare::PreparedRendering<'_>,
    links: &[(u64, HtmlHref)],
    budget: &mut Budget,
) -> Result<RenderedFragment, RenderError> {
    render_prepared_with_foreign(
        prepared,
        links,
        &mut |_, _, _| Err(RenderError::InternalShape),
        budget,
    )
    .map(|rendered| rendered.fragment)
    .map_err(|error| match error {
        ForeignRenderError::Render(error) | ForeignRenderError::Foreign(error) => error,
    })
}

/// Invoke the explicitly selected host adapter for each InlineMath occurrence.
/// The callback receives the closure from the immutable preparation input;
/// malformed/non-phrasing output and unresolved non-guest requirements fail.
pub fn render_inline_with_foreign<E>(
    prepared: &PreparedInlineWithForeign<'_>,
    adapter: &mut impl FnMut(&DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<RenderedInlineWithForeign, ForeignRenderError<E>> {
    render_prepared_with_foreign(&prepared.0, &[], adapter, budget)
}

fn render_prepared_with_foreign<E>(
    prepared: &crate::prepare::PreparedRendering<'_>,
    links: &[(u64, HtmlHref)],
    adapter: &mut impl FnMut(&DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<RenderedInlineWithForeign, ForeignRenderError<E>> {
    budget.poll()?;
    let mut w = Builder {
        prepared,
        links,
        b: budget,
        nodes: Vec::new(),
        depths: Vec::new(),
        origins: Vec::new(),
        jobs: Vec::new(),
        classes: Vec::new(),
        foreign: Vec::new(),
    };
    let (root, slot) = match prepared.document.value.root {
        DocRoot::Article(root) => (root.0, HtmlSlot::Block),
        DocRoot::Sentence(root) => (root.0, HtmlSlot::Phrasing),
        DocRoot::Inline(root) => (root.0, HtmlSlot::Phrasing),
        _ => return Err(RenderError::InternalShape.into()),
    };
    let article = w.element(
        None,
        root,
        if slot == HtmlSlot::Block {
            HtmlTag::Article
        } else {
            HtmlTag::Span
        },
    )?;
    if slot == HtmlSlot::Block {
        w.class(article, "nepl-doc")?;
        let DocKind::Article {
            language,
            title,
            body,
        } = &prepared.document.value.nodes[root as usize].kind
        else {
            return Err(RenderError::InternalShape.into());
        };
        let lang = copy(language, w.b)?;
        w.attr(article, HtmlAttribute::Lang { value: lang })?;
        w.job(body.0, article, 1)?;
        w.heading(article, root, title.0, 1)?;
    } else {
        w.class(article, "nepl-sentence")?;
        w.job(root, article, 1)?;
    }
    while let Some(job) = w.jobs.pop() {
        w.b.charge(Resource::Work, 1)?;
        let kind = &prepared.document.value.nodes[job.node as usize].kind;
        if let DocKind::InlineMath { syntax } = kind {
            let embed = prepared
                .document
                .value
                .embeds
                .get(syntax.0 as usize)
                .ok_or(RenderError::InternalShape)?;
            let depth =
                w.b.current_depth()
                    .saturating_add(w.depths[job.parent as usize]);
            let result = w
                .b
                .with_depth_at_least(depth, |b| Ok::<_, RenderError>(adapter(embed, *syntax, b)))?;
            w.b.poll()?;
            let markup = result.map_err(ForeignRenderError::Foreign)?;
            w.guest(job, *syntax, markup)?;
            continue;
        }
        if !w.block(job, kind)? {
            w.inline(job, kind)?;
        }
    }
    let mut classes = w.classes;
    for class in CLASSES {
        let mut present = false;
        for prior in &classes {
            w.b.charge(Resource::Work, prior.len().min(class.len()) as u64 + 1)?;
            if prior == class {
                present = true;
                break;
            }
        }
        if present {
            continue;
        }
        let value = copy(class, w.b)?;
        push(&mut classes, value, w.b)?;
    }
    let markup = HtmlRequest {
        fragment: HtmlFragment {
            root: article,
            nodes: w.nodes,
        },
        slot,
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
    Ok(RenderedInlineWithForeign {
        fragment: RenderedFragment {
            document_digest: prepared.identity,
            options: RenderOptions { parallel },
            markup,
            origins: w.origins,
        },
        foreign: w.foreign,
    })
}
