//! Compare resolved interpretation along the two independently authenticated paths.
use super::*;
use crate::analysis::probe::read;
use nepl3_core::{
    budget::Resource,
    syntax::{FieldValue, NodeRef, SyntaxBundle},
};

fn selected_equal(
    a: &PreparedBindingRequest<'_, '_>,
    ab: &SyntaxBundle,
    an: NodeRef,
    c: &PreparedBindingRequest<'_, '_>,
    cb: &SyntaxBundle,
    cn: NodeRef,
    b: &mut Budget,
) -> Result<bool, DeclarationError> {
    let (a, _) = read::context(a, ab, an, b).map_err(DeclarationError::Read)?;
    let (c, _) = read::context(c, cb, cn, b).map_err(DeclarationError::Read)?;
    b.charge(Resource::Work, 32)?;
    Ok(a.execution_digest == c.execution_digest
        && crate::selection::entry_equal(&a.entry, &c.entry, b)?
        && a.shape.same_resolved_with_budget(&c.shape, b)?)
}

pub(super) fn same(
    a: &PreparedBindingRequest<'_, '_>,
    old: &location::Location<'_>,
    c: &PreparedBindingRequest<'_, '_>,
    new: &location::Location<'_>,
    b: &mut Budget,
) -> Result<bool, DeclarationError> {
    b.charge(Resource::Work, 1)?;
    if old.path.len() != new.path.len() {
        return Ok(false);
    }
    let (mut ab, mut cb) = (&a.tree.tree().bundle, &c.tree.tree().bundle);
    let (mut an, mut cn) = (ab.root, cb.root);
    for (depth, (as_, cs)) in old.path.iter().zip(&new.path).enumerate() {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        b.observe_depth((depth as u64).saturating_add(1))?;
        if as_ != cs || !selected_equal(a, ab, an, c, cb, cn, b)? {
            return Ok(false);
        }
        let index = match as_ {
            ExpectedReadStep::Child { field } | ExpectedReadStep::Foreign { field } => *field,
        };
        let index = usize::try_from(index).map_err(|_| DeclarationError::Structure)?;
        let (sa, _) = read::context(a, ab, an, b).map_err(DeclarationError::Read)?;
        let (sc, _) = read::context(c, cb, cn, b).map_err(DeclarationError::Read)?;
        let ad =
            crate::tree::read::declared(sa, index, a.profile, b).map_err(DeclarationError::Tree)?;
        let cd =
            crate::tree::read::declared(sc, index, c.profile, b).map_err(DeclarationError::Tree)?;
        let ar =
            crate::tree::read::child(sa, index, a.profile, b).map_err(DeclarationError::Tree)?;
        let cr =
            crate::tree::read::child(sc, index, c.profile, b).map_err(DeclarationError::Tree)?;
        if ad != cd
            || ar.read != cr.read
            || ar.foreign != cr.foreign
            || !crate::selection::entry_equal(&ar.entry, &cr.entry, b)?
        {
            return Ok(false);
        }
        let af = usize::try_from(an.0)
            .ok()
            .and_then(|i| ab.nodes.get(i))
            .and_then(|n| n.fields.get(index))
            .ok_or(DeclarationError::Structure)?;
        let cf = usize::try_from(cn.0)
            .ok()
            .and_then(|i| cb.nodes.get(i))
            .and_then(|n| n.fields.get(index))
            .ok_or(DeclarationError::Structure)?;
        match (as_, af, cf) {
            (ExpectedReadStep::Child { .. }, FieldValue::Child(a), FieldValue::Child(c)) => {
                an = *a;
                cn = *c;
            }
            (ExpectedReadStep::Foreign { .. }, FieldValue::Foreign(a), FieldValue::Foreign(c)) => {
                if !foreign_equal(a, c, b)? {
                    return Ok(false);
                }
                ab = &a.bundle;
                cb = &c.bundle;
                an = ab.root;
                cn = cb.root;
            }
            _ => return Err(DeclarationError::Structure),
        }
    }
    if !core::ptr::eq(ab, old.bundle)
        || an != old.node
        || !core::ptr::eq(cb, new.bundle)
        || cn != new.node
    {
        return Err(DeclarationError::Structure);
    }
    selected_equal(a, ab, an, c, cb, cn, b)
}

fn foreign_equal(
    a: &nepl3_core::syntax::ForeignSyntax,
    c: &nepl3_core::syntax::ForeignSyntax,
    b: &mut Budget,
) -> Result<bool, DeclarationError> {
    b.charge(
        Resource::Work,
        (a.schema.package.len() as u64)
            .saturating_add(c.schema.package.len() as u64)
            .saturating_add(a.category.len() as u64)
            .saturating_add(c.category.len() as u64)
            .saturating_add(81),
    )?;
    Ok(a.schema == c.schema
        && a.category == c.category
        && a.environment.digest == c.environment.digest)
}
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use nepl3_core::{
        budget::Limits,
        source::Digest,
        syntax::{EnvironmentRef, ForeignSyntax},
        value::SchemaRef,
    };
    #[test]
    fn foreign_context_compares_schema_category_and_environment_digest()
    -> Result<(), DeclarationError> {
        // Isolated wrapper comparison; this empty bundle is not a parser acceptance fixture.
        let a = ForeignSyntax {
            schema: SchemaRef {
                package: "test.guest".into(),
                revision: 1,
                digest: Digest::of(b"schema"),
            },
            category: "Expr".into(),
            root: NodeRef(0),
            environment: EnvironmentRef {
                id: 0,
                digest: Digest::of(b"environment"),
            },
            bundle: SyntaxBundle {
                sources: Vec::new(),
                nodes: Vec::new(),
                origins: Vec::new(),
                root: NodeRef(0),
                environments: Vec::new(),
                tokens: Vec::new(),
                source_maps: Vec::new(),
            },
        };
        let limits = Limits {
            work: 100_000,
            ..Limits::default()
        };
        let mut c = a.clone();
        c.environment.id = 99;
        assert!(foreign_equal(&a, &c, &mut Budget::new(limits))?);
        c.environment.digest = Digest::of(b"other");
        assert!(!foreign_equal(&a, &c, &mut Budget::new(limits))?);
        c = a.clone();
        c.category = "Type".into();
        assert!(!foreign_equal(&a, &c, &mut Budget::new(limits))?);
        c = a.clone();
        c.schema.revision += 1;
        assert!(!foreign_equal(&a, &c, &mut Budget::new(limits))?);
        let mut measured = Budget::new(limits);
        assert!(foreign_equal(&a, &a, &mut measured)?);
        let mut stopped = Budget::new(Limits {
            work: measured.usage().work - 1,
            ..limits
        });
        assert!(matches!(
            foreign_equal(&a, &a, &mut stopped),
            Err(DeclarationError::Stopped(StopReason::WorkLimit))
        ));
        Ok(())
    }
}
