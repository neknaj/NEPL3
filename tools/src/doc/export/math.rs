//! Selected native Math display with retained source mappings. This host has no
//! admitted KaTeX execution capability yet; fallback diagnostics stay explicit.
use super::super::{annotations::MathRecord, math::{MathDisplayHost, ProjectionError, display::{Preference, TexPreparation}}};
use super::{Compiled, err};
use nepl3_core::{budget::{Budget, Resource, StopReason}, schema::SchemaRegistry, value_codec::FoundationValueCodec};
use nepl3_doc_core::model::{DocEmbed, EmbedKind, EmbedRef};
use nepl3_doc_html::ForeignPlacement;
use nepl3_markup::{html::HtmlRequest, mathml::Display};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fallback {
 HostCapabilityUnavailable,
 UnsupportedTex { node: u64, reason: nepl3_math_tex::Unsupported },
}
pub struct Occurrence {
 pub embed: EmbedRef,
 /// Final arena indices with the independent guest sources retained.
 pub output: MathRecord,
 pub fallback: Option<Fallback>,
}
pub struct DisplayHost<'a, C> {
 pub compiled: &'a Compiled,
 pub registry: &'a SchemaRegistry,
 pub codec: &'a mut C,
 pub preference: Preference,
}
impl<C: FoundationValueCodec> DisplayHost<'_, C> where C::Error: core::fmt::Debug {
 pub fn render(&mut self, guest: &DocEmbed, embed: EmbedRef, records: &mut Vec<Occurrence>, b: &mut Budget) -> Result<HtmlRequest, String> {
  b.poll().map_err(err)?;
  let display = match guest.kind {
   EmbedKind::InlineMath => Display::Inline,
   EmbedKind::DisplayMath => Display::Block,
   _ => return Err("MathSelection".into()),
  };
  let mut host = MathDisplayHost {
   registry: self.registry,
   math_surface: &self.compiled.others.first().ok_or("MathSelection")?.schema,
   sentence_surface: Some(&self.compiled.others.get(3).ok_or("SentenceSelection")?.schema),
   doc_surface: Some(&self.compiled.doc.package.schema), codec: self.codec,
  };
  let (mathml, tex) = host.prepare_guest(&guest.closure, display, self.preference, b).map_err(err)?.into_parts();
  let fallback = match tex {
   TexPreparation::MathMLOnly => None,
   TexPreparation::Unsupported { node, reason } => Some(Fallback::UnsupportedTex { node, reason }),
   TexPreparation::Ready { .. } => Some(Fallback::HostCapabilityUnavailable),
  };
  if fallback.is_some() { b.charge(Resource::Diagnostics, 1).map_err(err)?; }
  let result = mathml.into_html(b).map_err(err)?;
  b.charge(Resource::AllocationUnits, core::mem::size_of::<Occurrence>() as u64).map_err(err)?;
  records.try_reserve_exact(1).map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
  records.push(Occurrence { embed, output: MathRecord {
   syntax: result.syntax, node_roots: result.node_roots, annotation_roots: result.annotation_roots,
   annotations: result.annotations,
  }, fallback });
  Ok(result.markup)
 }
}
/// Traverse placements in output order; repeated guest occurrences stay distinct.
pub fn compose(records: &mut [Occurrence], placements: &[ForeignPlacement], embeds: &[DocEmbed], b: &mut Budget) -> Result<(), String> {
 let mut records = records.iter_mut();
 for placement in placements {
  b.charge(Resource::Work, 1).map_err(err)?;
  let guest = embeds.get(placement.embed.0 as usize).ok_or("MathPlacement")?;
  if guest.kind == EmbedKind::Code { continue; }
  let record = records.next().ok_or("MathPlacement")?;
  if record.embed != placement.embed { return Err("MathPlacement".into()); }
  record.output.remap(&mut |element| {
   if element >= placement.elements { return Err(ProjectionError::Mapping(element)); }
   placement.first_element.checked_add(element).ok_or(ProjectionError::Mapping(element))
  }, b).map_err(err)?;
 }
 if records.next().is_some() { return Err("MathPlacement".into()); }
 b.poll().map_err(err)
}
/// Bounded host manifest bookkeeping; not a portable runtime diagnostic schema.
pub fn diagnostics(records: &[Occurrence]) -> Vec<serde_json::Value> {
 records.iter().enumerate().filter_map(|(occurrence, record)| record.fallback.map(|reason| {
  serde_json::json!({"occurrence":occurrence,"embed":record.embed.0,"renderer":"MathML",
   "detail":match reason { Fallback::UnsupportedTex {node, reason} => Some(serde_json::json!({"node":node,"reason":format!("{reason:?}")})), _ => None },
   "reason":match reason { Fallback::HostCapabilityUnavailable => "KaTeXHostCapabilityUnavailable", Fallback::UnsupportedTex {..} => "FaithfulTexUnsupported" }})
 })).collect()
}
