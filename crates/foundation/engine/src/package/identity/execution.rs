//! Concrete index/table identity for suspension and provenance-bearing consumers.
use super::*;
use nepl3_core::{
    origin::{MappingKind, Origin},
    source::{SnapshotId, Span},
};
fn digest(out: &mut CanonicalWriter<'_>, v: Digest) -> Result<(), PackageError> {
    out.push("[")?;
    for (i, b) in v.0.iter().enumerate() {
        comma(out, i)?;
        out.number(u64::from(*b))?;
    }
    out.push("]")?;
    Ok(())
}
fn source(out: &mut CanonicalWriter<'_>, s: &SnapshotId) -> Result<(), PackageError> {
    out.push("[")?;
    out.quoted(&s.source.0)?;
    out.push(",")?;
    out.number(s.revision)?;
    out.push(",")?;
    digest(out, s.digest)?;
    out.push("]")?;
    Ok(())
}
fn span(out: &mut CanonicalWriter<'_>, s: &Span) -> Result<(), PackageError> {
    out.push("[")?;
    source(out, s.snapshot_ref())?;
    out.push(",")?;
    out.number(s.start())?;
    out.push(",")?;
    out.number(s.end())?;
    out.push("]")?;
    Ok(())
}
fn optional_span(out: &mut CanonicalWriter<'_>, s: Option<&Span>) -> Result<(), PackageError> {
    if let Some(s) = s {
        span(out, s)?;
    } else {
        out.push("null")?;
    }
    Ok(())
}
impl CheckedLanguagePackage<'_> {
    /// Includes concrete arena indices and all source/provenance tables.
    /// This identity is not the Grammar bootstrap semantic comparison.
    pub fn execution_digest(&self, budget: &mut Budget) -> Result<Digest, PackageError> {
        let semantic = self.semantic_json(budget)?;
        reader::charge_plan_sort(&self.package.reader, budget)?;
        let concrete_reader = self.package.reader.canonical_json(budget)?;
        let p = self.package;
        let mut out = CanonicalWriter::new(budget);
        out.push("{\"bindings\":[")?;
        for (i, b) in p.bindings.iter().enumerate() {
            comma(&mut out, i)?;
            out.push("[")?;
            binding(&mut out, p, BindingId(i as u64))?;
            out.push(",[")?;
            if let Binding::Group(ids) | Binding::Scope(ids) = b {
                for (j, id) in ids.iter().enumerate() {
                    comma(&mut out, j)?;
                    out.number(id.0)?;
                }
            }
            out.push("]]")?;
        }
        out.push("],\"layout\":[[")?;
        for (i, f) in p.forms.iter().enumerate() {
            comma(&mut out, i)?;
            out.push("[")?;
            out.quoted(&f.category)?;
            out.push(",")?;
            out.quoted(&f.spelling)?;
            out.push(",")?;
            out.number(f.binding.0)?;
            out.push(",[")?;
            for (j, v) in f.fields.iter().enumerate() {
                comma(&mut out, j)?;
                out.number(v.read.0)?;
            }
            out.push("]]")?;
        }
        out.push("],[")?;
        for (i, l) in p.leaves.iter().enumerate() {
            comma(&mut out, i)?;
            out.push("[")?;
            out.quoted(&l.category)?;
            out.push(",")?;
            kind(&mut out, &l.token_kind)?;
            out.push(",")?;
            out.number(l.binding.0)?;
            out.push("]")?;
        }
        out.push("],[")?;
        for (i, m) in p.modes.iter().enumerate() {
            comma(&mut out, i)?;
            out.quoted(&m.name)?;
        }
        out.push("],[")?;
        for (i, c) in p.categories.iter().enumerate() {
            comma(&mut out, i)?;
            out.quoted(&c.name)?;
        }
        out.push("],[")?;
        for (i, n) in p.namespaces.iter().enumerate() {
            comma(&mut out, i)?;
            out.quoted(&n.name)?;
        }
        out.push("],[")?;
        for (i, e) in p.extensions.iter().enumerate() {
            comma(&mut out, i)?;
            out.quoted(&e.alias)?;
        }
        out.push("]],\"provenance\":[[")?;
        for (i, s) in p.provenance.sources.iter().enumerate() {
            comma(&mut out, i)?;
            out.push("[")?;
            source(&mut out, s.identity())?;
            out.push(",")?;
            out.quoted(s.uri())?;
            out.push("]")?;
        }
        out.push("],[")?;
        for (i, o) in p.provenance.origins.iter().enumerate() {
            comma(&mut out, i)?;
            match o {
                Origin::Direct(s) => {
                    out.push("[\"Direct\",")?;
                    span(&mut out, s)?;
                    out.push("]")?;
                }
                Origin::Composite(ids) => {
                    out.push("[\"Composite\",[")?;
                    for (j, id) in ids.iter().enumerate() {
                        comma(&mut out, j)?;
                        out.number(id.0)?;
                    }
                    out.push("]]")?;
                }
                Origin::Generated {
                    operation: v,
                    callsite,
                    inputs,
                } => {
                    out.push("[\"Generated\",")?;
                    operation(&mut out, v)?;
                    out.push(",")?;
                    optional_span(&mut out, callsite.as_ref())?;
                    out.push(",[")?;
                    for (j, id) in inputs.iter().enumerate() {
                        comma(&mut out, j)?;
                        out.number(id.0)?;
                    }
                    out.push("]]")?;
                }
                Origin::Synthetic { reason, anchor } => {
                    out.push("[\"Synthetic\",")?;
                    out.quoted(reason)?;
                    out.push(",")?;
                    optional_span(&mut out, anchor.as_ref())?;
                    out.push("]")?;
                }
            }
        }
        out.push("],[")?;
        for (i, m) in p.provenance.source_maps.iter().enumerate() {
            comma(&mut out, i)?;
            out.push("[")?;
            span(&mut out, &m.source)?;
            out.push(",")?;
            span(&mut out, &m.target)?;
            out.push(",")?;
            out.quoted(match m.kind {
                MappingKind::Exact => "Exact",
                MappingKind::Transformed => "Transformed",
            })?;
            out.push("]")?;
        }
        out.push("],[")?;
        for (i, d) in p.provenance.declarations.iter().enumerate() {
            comma(&mut out, i)?;
            out.push("[")?;
            out.quoted(match d.kind {
                DeclarationKind::Category => "Category",
                DeclarationKind::Mode => "Mode",
                DeclarationKind::Reader => "Reader",
                DeclarationKind::Form => "Form",
                DeclarationKind::Leaf => "Leaf",
                DeclarationKind::Namespace => "Namespace",
                DeclarationKind::Extension => "Extension",
            })?;
            out.push(",")?;
            out.quoted(&d.name)?;
            out.push(",")?;
            out.number(d.origin.0)?;
            out.push("]")?;
        }
        out.push("]],\"reader\":")?;
        out.push(core::str::from_utf8(&concrete_reader).map_err(|_| PackageError::KindShape)?)?;
        out.push(",\"reads\":[")?;
        for (i, r) in p.reads.iter().enumerate() {
            comma(&mut out, i)?;
            out.push("[")?;
            read(&mut out, p, ReadSpecId(i as u64))?;
            out.push(",")?;
            match r {
                ReadSpec::WithMode { read, .. } => out.number(read.0)?,
                ReadSpec::ListOf { element, .. } => out.number(element.0)?,
                _ => out.push("null")?,
            }
            out.push("]")?;
        }
        out.push("],\"semantics\":")?;
        out.push(core::str::from_utf8(&semantic).map_err(|_| PackageError::KindShape)?)?;
        out.push("}")?;
        let bytes = out.finish();
        budget.charge(Resource::Work, bytes.len() as u64)?;
        Ok(Digest::domain(b"NEPL3-PACKAGE-EXECUTION-1\0", &bytes))
    }
}
