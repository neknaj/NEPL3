//! Root-only embedded-SVG packaging, immutable-source CLI export and native
//! retained source preparation and native render retry. The plan-based renderer
//! itself does not parse or generate KaTeX. Explicit CLI continuation reuses the
//! native jobs; durable restoration remains separate.
pub mod cli;
mod generation;
pub mod identity;
mod input;
mod output;
pub mod preparation;
use crate::doc::annotations::article::render::svg::pages as planned;
use crate::doc::annotations::{SentenceAnnotationRenderer, article::display::Plan};
use nepl3_core::{
    budget::{Budget, StopReason},
    source::Digest,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    model::{DocEmbedView, EmbedRef},
    pages::PageSet,
};
use nepl3_doc_html::RenderOptions;
use nepl3_markup::html::HtmlRequest;
pub use planned::RootAssets;
use std::collections::BTreeMap;

/// The caller must retain the PageSet, options and selections for this lifetime.
/// The rendered result retains adopted owners, Sentence records and alt sources.
pub struct Packaged<'s, 'a> {
    pub rendered: planned::Rendered<'s, 'a>,
    pub input_identity: Digest,
    pub files: BTreeMap<String, Vec<u8>>,
    /// Completion receipt kept separate for a future writer to publish last.
    pub manifest: String,
}
#[derive(Debug)]
pub enum Error<E, F> {
    Stopped(StopReason),
    Render(Box<planned::Error<E, F>>),
    Packaging(String),
}

/// Renders and packages all pages before returning any completed output.
/// SVG admission remains root-only. This wrapper preserves the exact callback
/// output and does not execute KaTeX or verify mathematical statements. Input
/// identity is not a callback identity; the receipt hashes every actual file.
#[allow(clippy::too_many_arguments)]
pub fn render_embedded<'s, 'a, C: FoundationValueCodec, F>(
    set: &'a PageSet,
    options: &'s RenderOptions,
    plans: &[Plan<'s, 'a>],
    inputs: &[RootAssets<'_>],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    other: &mut impl FnMut(
        u64,
        &DocEmbedView<'_>,
        EmbedRef,
        &mut C,
        &mut Budget,
    ) -> Result<HtmlRequest, F>,
    budget: &mut Budget,
) -> Result<Packaged<'s, 'a>, Error<C::Error, F>>
where
    C::Error: core::fmt::Debug,
{
    let initial = budget.usage();
    let result = (|| {
        budget.poll().map_err(Error::Stopped)?;
        let rendered = match planned::render(set, options, plans, inputs, host, other, budget) {
            Ok(value) => value,
            Err(error) => {
                budget
                    .charge(
                        nepl3_core::budget::Resource::AllocationUnits,
                        core::mem::size_of_val(&error) as u64,
                    )
                    .map_err(Error::Stopped)?;
                return Err(Error::Render(Box::new(error)));
            }
        };
        let input_identity =
            identity::input(set, options, inputs, host.registry, host.codec, budget)
                .map_err(Error::Packaging)?;
        let (files, manifest) = output::package(
            set,
            options,
            inputs,
            &rendered,
            input_identity,
            initial,
            None,
            None,
            budget,
        )
        .map_err(Error::Packaging)?;
        Ok(Packaged {
            rendered,
            input_identity,
            files,
            manifest,
        })
    })();
    budget.poll().map_err(Error::Stopped)?;
    result
}
