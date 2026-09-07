use super::*;
use crate::{
    recovery::ParseTree,
    selection::{BundleContext, ShapeSelection},
};
use nepl3_core::{
    syntax::{FieldValue, SyntaxBundle, canonical::BundleMappings},
    value::{NdfScalar, NdfValue},
};
fn canonical(error: nepl3_core::syntax::canonical::CanonicalError) -> RenameError {
    match error {
        nepl3_core::syntax::canonical::CanonicalError::Stopped(r) => RenameError::Stopped(r),
        _ => RenameError::ShapeChanged,
    }
}
pub(super) fn check(
    d: &mut RenameDraft<'_, '_, '_, '_>,
    p: &PreparedBindingRequest<'_, '_>,
    old: &FactSet,
    new: &FactSet,
    owners: (&super::owner::Owners, &super::owner::Owners),
) -> Result<(), RenameError> {
    let a = BundleMappings::new(&d.original.tree.tree().bundle, d.budget).map_err(canonical)?;
    let c = BundleMappings::new(&p.tree.tree().bundle, d.budget).map_err(canonical)?;
    if a.entries().len() != c.entries().len() {
        return Err(RenameError::ShapeChanged);
    }
    for (index, (a, c)) in a.entries().iter().zip(c.entries()).enumerate() {
        let maps = (owners.0.bundle(index)?, owners.1.bundle(index)?);
        let ab = a.bundle();
        let cb = c.bundle();
        if a.order().len() != c.order().len() || ab.environments.len() != cb.environments.len() {
            return Err(RenameError::ShapeChanged);
        }
        for (a, c) in ab.environments.iter().zip(&cb.environments) {
            d.budget.charge(Resource::Work, 42)?;
            if a.id != c.id || a.digest != c.digest {
                return Err(RenameError::ShapeChanged);
            }
        }
        let ac = context(
            d.original.tree.tree(),
            ab,
            d.original.profile.registry(),
            d.budget,
        )?;
        let cc = context(p.tree.tree(), cb, p.profile.registry(), d.budget)?;
        for (ai, ci) in a.order().iter().zip(c.order()) {
            d.budget.charge(Resource::Nodes, 1)?;
            let an = &ab.nodes[*ai];
            let cn = &cb.nodes[*ci];
            d.budget.charge(
                Resource::Work,
                (an.schema.package.len() + cn.schema.package.len() + an.kind.len() + cn.kind.len())
                    as u64
                    + 42,
            )?;
            if an.schema != cn.schema || an.kind != cn.kind || an.fields.len() != cn.fields.len() {
                return Err(RenameError::ShapeChanged);
            }
            for (a, c) in [(&an.head, &cn.head), (&an.cover, &cn.cover)] {
                match (a, c) {
                    (Some(a), Some(c)) if super::verify::spans(d, old, new, a, c, maps)? => {}
                    (None, None) => {}
                    _ => return Err(RenameError::ShapeChanged),
                }
            }
            d.budget
                .charge(Resource::Work, (ac.nodes.len() + cc.nodes.len()) as u64)?;
            let av = ac
                .nodes
                .iter()
                .find(|v| v.node.0 == *ai as u64)
                .ok_or(RenameError::ShapeChanged)?;
            let cv = cc
                .nodes
                .iter()
                .find(|v| v.node.0 == *ci as u64)
                .ok_or(RenameError::ShapeChanged)?;
            d.budget.charge(
                Resource::Work,
                (av.entry.alias.len()
                    + cv.entry.alias.len()
                    + av.entry.category.len()
                    + cv.entry.category.len()
                    + av.entry.mode.len()
                    + cv.entry.mode.len()) as u64
                    + 42,
            )?;
            if av.entry != cv.entry
                || av.execution_digest != cv.execution_digest
                || !selection(&av.shape, &cv.shape, d.budget)?
            {
                return Err(RenameError::ShapeChanged);
            }
            let edited = an
                .head
                .as_ref()
                .map(|span| is_edit(d, span))
                .transpose()?
                .unwrap_or(false);
            for (af, cf) in an.fields.iter().zip(&cn.fields) {
                let same = match (af, cf) {
                    (FieldValue::Child(x), FieldValue::Child(y)) => {
                        a.mapped(*x).map_err(canonical)? == c.mapped(*y).map_err(canonical)?
                    }
                    (FieldValue::Children(x), FieldValue::Children(y)) => {
                        if x.len() != y.len() {
                            return Err(RenameError::ShapeChanged);
                        }
                        for (x, y) in x.iter().zip(y) {
                            d.budget.charge(Resource::Work, 1)?;
                            if a.mapped(*x).map_err(canonical)?
                                != c.mapped(*y).map_err(canonical)?
                            {
                                return Err(RenameError::ShapeChanged);
                            }
                        }
                        true
                    }
                    (FieldValue::Foreign(x), FieldValue::Foreign(y)) => {
                        d.budget.charge(
                            Resource::Work,
                            (x.schema.package.len()
                                + y.schema.package.len()
                                + x.category.len()
                                + y.category.len()) as u64
                                + 84,
                        )?;
                        x.schema == y.schema
                            && x.category == y.category
                            && x.environment == y.environment
                    }
                    (FieldValue::Atom(x), FieldValue::Atom(y)) => {
                        atom(x, y, edited, &d.request.new_name, d.budget)?
                    }
                    _ => false,
                };
                if !same {
                    return Err(RenameError::ShapeChanged);
                }
            }
            match (an.token, cn.token) {
                (Some(x), Some(y)) => {
                    let x = ab
                        .tokens
                        .get(x.0 as usize)
                        .ok_or(RenameError::ShapeChanged)?;
                    let y = cb
                        .tokens
                        .get(y.0 as usize)
                        .ok_or(RenameError::ShapeChanged)?;
                    d.budget.charge(
                        Resource::Work,
                        (x.kind.schema.package.len() + y.kind.schema.package.len()) as u64 + 42,
                    )?;
                    if x.kind != y.kind
                        || !super::verify::spans(d, old, new, &x.head, &y.head, maps)?
                    {
                        return Err(RenameError::ShapeChanged);
                    }
                    if let NdfValue::Text(value) = &y.payload {
                        d.budget.charge(
                            Resource::Work,
                            (value.len() + d.request.new_name.len()) as u64,
                        )?;
                    }
                    let renamed = edited
                        && matches!((&x.payload,&y.payload),(NdfValue::Text(_),NdfValue::Text(value)) if value==&d.request.new_name);
                    if !renamed && !x.payload.equal_with_budget(&y.payload, d.budget)? {
                        return Err(RenameError::ShapeChanged);
                    }
                }
                (None, None) => {}
                _ => return Err(RenameError::ShapeChanged),
            }
        }
    }
    Ok(())
}
fn context<'a>(
    tree: &'a ParseTree,
    bundle: &SyntaxBundle,
    registry: &nepl3_core::schema::SchemaRegistry,
    b: &mut Budget,
) -> Result<&'a BundleContext, RenameError> {
    for context in &tree.contexts {
        let owner =
            crate::tree::path(&tree.bundle, &context.path, registry, b).map_err(|e| match e {
                crate::tree::TreeError::Stopped(r) => RenameError::Stopped(r),
                _ => RenameError::ShapeChanged,
            })?;
        if core::ptr::eq(owner, bundle) {
            return Ok(context);
        }
    }
    Err(RenameError::ShapeChanged)
}
fn selection(a: &ShapeSelection, c: &ShapeSelection, b: &mut Budget) -> Result<bool, RenameError> {
    b.charge(Resource::Work, 1)?;
    Ok(match (a, c) {
        (ShapeSelection::Form { index: a }, ShapeSelection::Form { index: c })
        | (ShapeSelection::Leaf { index: a }, ShapeSelection::Leaf { index: c }) => a == c,
        (ShapeSelection::Builtin { read: a }, ShapeSelection::Builtin { read: c }) => a == c,
        (ShapeSelection::List { read: a, cons: x }, ShapeSelection::List { read: c, cons: y }) => {
            a == c && x == y
        }
        (
            ShapeSelection::Dynamic {
                provider: a,
                shape: x,
                child_contexts: u,
            },
            ShapeSelection::Dynamic {
                provider: c,
                shape: y,
                child_contexts: v,
            },
        ) => {
            for operation in [&a.shape, &a.child_context, &c.shape, &c.child_context] {
                b.charge(
                    Resource::Work,
                    (operation.schema.package.len() + operation.name.len()) as u64 + 42,
                )?;
            }
            for shape in [x, y] {
                b.charge(Resource::Work, shape.kind.schema.package.len() as u64 + 42)?;
                for field in &shape.fields {
                    b.charge(Resource::Work, field.name.len() as u64 + 9)?;
                }
                for rule in &shape.selection_rules {
                    let size = match &rule.selector {
                        crate::package::StyleSelector::Field(v)
                        | crate::package::StyleSelector::Capture(v) => v.len(),
                        _ => 0,
                    };
                    b.charge(Resource::Work, size as u64 + 9)?;
                }
                for style in &shape.styles {
                    use crate::package::StyleSelector;
                    let size = match &style.selector {
                        StyleSelector::Field(v) | StyleSelector::Capture(v) => v.len(),
                        _ => 0,
                    };
                    b.charge(
                        Resource::Work,
                        (size + style.class.schema.package.len() + style.class.name.len()) as u64
                            + 43,
                    )?;
                }
            }
            for entry in u.iter().chain(v) {
                b.charge(
                    Resource::Work,
                    (entry.alias.len() + entry.category.len() + entry.mode.len()) as u64 + 1,
                )?;
            }
            a == c && x == y && u == v
        }
        _ => false,
    })
}
fn is_edit(d: &mut RenameDraft<'_, '_, '_, '_>, span: &Span) -> Result<bool, RenameError> {
    for edit in &d.candidate_edits {
        if same_source(span.snapshot_ref(), edit.span.snapshot_ref(), d.budget)?
            && span.start() == edit.span.start()
            && span.end() == edit.span.end()
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn atom(
    a: &NdfScalar,
    c: &NdfScalar,
    edited: bool,
    new_name: &str,
    b: &mut Budget,
) -> Result<bool, RenameError> {
    let size = |v: &NdfScalar| match v {
        NdfScalar::Text(v) => v.len() as u64,
        NdfScalar::Bytes(v) => v.len() as u64,
        NdfScalar::Integer(v) => v.as_bigint().bits().div_ceil(8),
        NdfScalar::Rational(v) => {
            v.numerator().as_bigint().bits().div_ceil(8) + v.denominator().bits().div_ceil(8)
        }
        _ => 1,
    };
    b.charge(
        Resource::Work,
        size(a)
            .saturating_add(size(c))
            .saturating_add(new_name.len() as u64),
    )?;
    Ok(
        a == c
            || (edited && matches!((a,c),(NdfScalar::Text(_),NdfScalar::Text(v)) if v==new_name)),
    )
}
