//! Closed native Doc import; not a complete artifact or portable provider receipt.
use super::{ProjectionError, occurrence};
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_doc_core::model::EmbedKind;
use nepl3_doc_html::{ForeignRenderError, RenderError, RenderedInlineWithForeign, guests};
use nepl3_markup::html::HtmlRequest;

mod imported;
pub use imported::Imported;

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Adapter(E),
    Render(RenderError),
    Mapping(ProjectionError),
    Association,
    OccurrenceLimit { max: u64 },
    ReservedClass,
    DuplicateScope { previous: u64, current: u64 },
    ResourcesMismatch,
}
impl<E> From<StopReason> for Error<E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
/// Native output tied to the exact preparation, with globally remapped Math
/// indices. No mutable access or arbitrary output/source pairing is provided.
/// CSS/fonts still require metered artifact assembly and browser qualification.
/// ```compile_fail
/// fn mutate(d: &mut nepl3_tools::doc::math::document::LocalDocument<'_, '_, '_, '_>) { d.rendered.fragment.markup.fragment.nodes.clear(); }
/// ```
/// ```compile_fail
/// fn duplicate(d: nepl3_tools::doc::math::document::LocalDocument<'_, '_, '_, '_>) { d.clone(); }
/// ```
/// ```compile_fail
/// fn detach(d: nepl3_tools::doc::math::document::LocalDocument<'_, '_, '_, '_>) { d.into_parts(); }
/// ```
/// ```compile_fail
/// fn change(d: &mut nepl3_tools::doc::math::document::LocalDocument<'_, '_, '_, '_>) { d.math()[0].source().node_roots.clear(); }
/// ```
pub struct LocalDocument<'p, 'd, 'c, 'r> {
    prepared: &'p guests::PreparedArticle<'d>,
    rendered: RenderedInlineWithForeign,
    math: Vec<Imported<'d, 'c, 'r>>,
}
impl<'p, 'd, 'c, 'r> LocalDocument<'p, 'd, 'c, 'r> {
    pub fn prepared(&self) -> &'p guests::PreparedArticle<'d> {
        self.prepared
    }
    pub fn rendered(&self) -> &RenderedInlineWithForeign {
        &self.rendered
    }
    pub fn math(&self) -> &[Imported<'d, 'c, 'r>] {
        &self.math
    }
}
fn reject_reserved<E>(request: &HtmlRequest, b: &mut Budget) -> Result<(), Error<E>> {
    for class in &request.policy.classes {
        b.charge(Resource::Work, class.len() as u64 + 1)?;
        if class.starts_with("nepl-math-") {
            return Err(Error::ReservedClass);
        }
    }
    Ok(())
}
fn push<'d, 'c, 'r, E>(
    v: &mut Vec<Imported<'d, 'c, 'r>>,
    value: Imported<'d, 'c, 'r>,
    b: &mut Budget,
) -> Result<(), Error<E>> {
    b.charge(
        Resource::Work,
        core::mem::size_of::<Imported<'d, 'c, 'r>>() as u64,
    )?;
    if v.len() == v.capacity() {
        let cap = v
            .capacity()
            .checked_mul(2)
            .map(|n| n.max(1))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        let bytes = cap
            .checked_mul(core::mem::size_of::<Imported<'d, 'c, 'r>>())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        b.charge(
            Resource::Work,
            (v.len() * core::mem::size_of::<Imported<'d, 'c, 'r>>()) as u64,
        )?;
        v.try_reserve_exact(cap - v.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    v.push(value);
    Ok(())
}
/// Each adapter must use the supplied cumulative Budget honestly. Math output
/// is checked against the actual native input association; this does not add a
/// request epoch/freshness proof or qualify renderer execution/fidelity.
/// Ordinary adapter failures remain failures; no implicit fallback is selected.
/// max_math counts selected Doc Math callbacks; nested annotation work uses
/// the same Budget but is not a separate callback counted by this cap.
pub fn render<'p, 'd, 'c, 'r, E>(
    prepared: &'p guests::PreparedArticle<'d>,
    max_math: u64,
    math_adapter: &mut impl FnMut(
        guests::Context<'d>,
        &mut Budget,
    ) -> Result<occurrence::Composed<'d, 'c, 'r>, E>,
    code_adapter: &mut impl FnMut(guests::Context<'d>, &mut Budget) -> Result<HtmlRequest, E>,
    b: &mut Budget,
) -> Result<LocalDocument<'p, 'd, 'c, 'r>, Error<E>> {
    b.poll()?;
    let mut math: Vec<Imported<'d, 'c, 'r>> = Vec::new();
    let rendered = guests::render_with_context(
        prepared,
        &mut |context, b| {
            b.charge(Resource::Work, 1)?;
            if context.embed().kind == EmbedKind::Code {
                let request = code_adapter(context, b);
                b.poll()?;
                let request = request.map_err(Error::Adapter)?;
                reject_reserved(&request, b)?;
                return Ok(request);
            }
            if !matches!(
                context.embed().kind,
                EmbedKind::InlineMath | EmbedKind::DisplayMath
            ) {
                return Err(Error::Association);
            }
            if math.len() as u64 >= max_math {
                return Err(Error::OccurrenceLimit { max: max_math });
            }
            let expected = (
                context.document(),
                context.options(),
                context.embed(),
                context.document_digest(),
                context.node(),
                context.reference(),
                context.ordinal(),
            );
            let composed = math_adapter(context, b);
            b.poll()?;
            let composed = composed.map_err(Error::Adapter)?;
            let actual = composed.context();
            b.charge(Resource::Work, 71)?;
            if !core::ptr::eq(actual.document(), expected.0)
                || !core::ptr::eq(actual.options(), expected.1)
                || !core::ptr::eq(actual.embed(), expected.2)
                || actual.document_digest() != expected.3
                || actual.node() != expected.4
                || actual.reference() != expected.5
                || actual.ordinal() != expected.6
                || composed.math().doc_node() != expected.4
            {
                return Err(Error::Association);
            }
            for prior in &math {
                b.charge(Resource::Work, 1)?;
                if let (Some(old), Some(new)) = (prior.assets(), composed.math().assets()) {
                    b.charge(Resource::Work, 64)?;
                    if old.identity() != new.identity() {
                        return Err(Error::ResourcesMismatch);
                    }
                    b.charge(
                        Resource::Work,
                        (prior
                            .scope()
                            .len()
                            .min(composed.math().selected_scope().len())
                            + 1) as u64,
                    )?;
                    if prior.scope() == composed.math().selected_scope() {
                        return Err(Error::DuplicateScope {
                            previous: prior.context().ordinal(),
                            current: expected.6,
                        });
                    }
                }
            }
            let (context, parts) = composed.into_import_parts();
            if parts.metadata.assets.is_none() {
                reject_reserved(&parts.markup, b)?;
            }
            let original_nodes = parts.markup.fragment.nodes.len() as u64;
            let item = Imported {
                context,
                source: parts.math,
                metadata: parts.metadata,
                original_nodes,
                placement: None,
            };
            push(&mut math, item, b)?;
            Ok(parts.markup)
        },
        b,
    )
    .map_err(|error| match error {
        ForeignRenderError::Foreign(error) => error,
        ForeignRenderError::Render(RenderError::Stopped(s)) => Error::Stopped(s),
        ForeignRenderError::Render(error) => Error::Render(error),
    })?;
    b.poll()?;
    for item in &mut math {
        item.bind(&rendered, b)?;
    }
    b.poll()?;
    Ok(LocalDocument {
        prepared,
        rendered,
        math,
    })
}

pub mod bundle;

mod termination;
pub use termination::TerminationError;
