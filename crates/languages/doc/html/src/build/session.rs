//! Private suspension of the actual traversal; no guest placeholders or
//! visitor-only output is admitted as a completed document.
use super::*;
struct Pending {
    target: Job,
    reference: EmbedRef,
    slot: HtmlSlot,
    depth: u64,
}
#[must_use]
pub(super) enum Step<'d> {
    Guest(crate::guests::Context<'d>),
    Waiting,
    Finished(RenderedInlineWithForeign),
}
#[must_use]
pub(super) struct Session<'p, 'd, 'b> {
    prepared: &'p crate::prepare::PreparedRendering<'d>,
    builder: Option<Builder<'p, 'b>>,
    pending: Option<Pending>,
    root: u64,
    slot: HtmlSlot,
    validate_final: bool,
}
impl<'p, 'd, 'b> Session<'p, 'd, 'b> {
    pub(super) fn new(
        prepared: &'p crate::prepare::PreparedRendering<'d>,
        links: &'p [(u64, HtmlHref)],
        budget: &'b mut Budget,
        validate_final: bool,
    ) -> Result<Self, RenderError> {
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
            _ => return Err(RenderError::InternalShape),
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
                return Err(RenderError::InternalShape);
            };
            let lang = copy(language, w.b)?;
            w.attr(article, HtmlAttribute::Lang { value: lang })?;
            w.job(body.0, article, 1)?;
            w.heading(article, root, title.0, 1)?;
        } else {
            w.class(article, "nepl-sentence")?;
            w.job(root, article, 1)?;
        }
        Ok(Self {
            prepared,
            builder: Some(w),
            pending: None,
            root: article,
            slot,
            validate_final,
        })
    }
    pub(super) fn advance(&mut self) -> Result<Step<'d>, RenderError> {
        let mut w = self.builder.take().ok_or(RenderError::InternalShape)?;
        w.b.poll()?;
        if self.pending.is_some() {
            self.builder = Some(w);
            return Ok(Step::Waiting);
        }
        let prepared = self.prepared;
        while let Some(job) = w.jobs.pop() {
            w.b.charge(Resource::Work, 1)?;
            let kind = &prepared.document.value.nodes[job.node as usize].kind;
            if let DocKind::InlineMath { syntax }
            | DocKind::DisplayMath { syntax }
            | DocKind::Code { syntax } = kind
            {
                let embed = prepared
                    .document
                    .value
                    .embeds
                    .get(syntax.0 as usize)
                    .ok_or(RenderError::InternalShape)?;
                // Build structural wrappers before the callback, so both temporary
                // renderer traversal and imported markup use the actual parent depth.
                let target = if matches!(kind, DocKind::Code { .. }) {
                    let figure = w.element(Some(job.parent), job.node, HtmlTag::Figure)?;
                    let pre = w.element(Some(figure), job.node, HtmlTag::Pre)?;
                    let code = w.element(Some(pre), job.node, HtmlTag::Code)?;
                    Job {
                        parent: code,
                        ..job
                    }
                } else if matches!(kind, DocKind::DisplayMath { .. }) {
                    let display = w.element(Some(job.parent), job.node, HtmlTag::Div)?;
                    Job {
                        parent: display,
                        ..job
                    }
                } else {
                    job
                };
                let depth =
                    w.b.current_depth()
                        .saturating_add(w.depths[target.parent as usize]);
                let context = crate::guests::Context::new(
                    prepared.document,
                    prepared.options,
                    prepared.identity,
                    job.node,
                    *syntax,
                    embed,
                    w.foreign.len() as u64,
                );
                let slot = if matches!(kind, DocKind::DisplayMath { .. }) {
                    HtmlSlot::Block
                } else {
                    HtmlSlot::Phrasing
                };
                self.pending = Some(Pending {
                    target,
                    reference: *syntax,
                    slot,
                    depth,
                });
                self.builder = Some(w);
                return Ok(Step::Guest(context));
            }
            if !w.block(job, kind)? {
                w.inline(job, kind)?;
            }
        }
        finish(prepared, w, self.root, self.slot, self.validate_final).map(Step::Finished)
    }
    // As in the former synchronous callback, a post-callback stop discards R.
    // Native resources must be retained outside R before that boundary.
    pub(super) fn with_pending<T, E>(
        &mut self,
        operation: impl FnOnce(&mut Budget) -> Result<T, E>,
    ) -> Result<T, ForeignRenderError<E>> {
        let depth = self
            .pending
            .as_ref()
            .ok_or(RenderError::InternalShape)?
            .depth;
        let result = {
            let w = self.builder.as_mut().ok_or(RenderError::InternalShape)?;
            w.b.with_depth_at_least(depth, |b| Ok::<_, RenderError>(operation(b)))
                .and_then(|result| {
                    w.b.poll()?;
                    Ok(result)
                })
        };
        let result = result
            .map_err(ForeignRenderError::Render)
            .and_then(|result| result.map_err(ForeignRenderError::Foreign));
        if result.is_err() {
            self.builder = None;
            self.pending = None;
        }
        result
    }
    pub(super) fn resume(&mut self, markup: HtmlRequest) -> Result<(), RenderError> {
        let mut w = self.builder.take().ok_or(RenderError::InternalShape)?;
        w.b.poll()?;
        let pending = self.pending.take().ok_or(RenderError::InternalShape)?;
        w.guest(pending.target, pending.reference, markup, pending.slot)?;
        self.builder = Some(w);
        Ok(())
    }
}
fn finish(
    prepared: &crate::prepare::PreparedRendering<'_>,
    w: Builder<'_, '_>,
    article: u64,
    slot: HtmlSlot,
    validate_final: bool,
) -> Result<RenderedInlineWithForeign, RenderError> {
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
    if validate_final {
        validate(&markup.fragment, markup.slot, &markup.policy, w.b)?;
    }
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

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
