use super::*;
use crate::{
    package::{FieldSpec, SelectionRule, StyleRule, StyleSelector},
    selection::ShapeSelection,
};
use nepl3_core::{
    syntax::{FieldValue, NodeRef, canonical::BundleMappings},
    view::{ViewBundle, ViewRef},
};
use nepl3_reader::model::ReaderFact;

pub(super) fn run(
    input: &PreparedRegionInput<'_, '_, '_>,
    request: &RegionRequest,
    sources: &mut Vec<SourceSnapshot>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<RegionOutcome, RegionError> {
    b.charge(Resource::Work, 160)?;
    if request.key != input.key {
        return Err(RegionError::Access(BindingAccessError::StaleAnalysis));
    }
    let maps = check::mappings(&input.binding.tree.tree().bundle, b)?;
    let mut found = false;
    for entry in maps.entries() {
        for source in &entry.bundle().sources {
            b.charge(
                Resource::Work,
                (source.identity().source.0.len() + request.source.source_id.0.len()) as u64 + 42,
            )?;
            if source.identity().source == request.source.source_id
                && source.identity().revision == request.source.revision
                && source.identity().digest == request.source.digest
            {
                source.check_range(request.offset, request.offset)?;
                found = true;
            }
            admission.admit_existing(source, b)?;
            let mut existing = false;
            for prior in sources.iter() {
                b.charge(
                    Resource::Work,
                    (prior.identity().source.0.len() + source.identity().source.0.len()) as u64
                        + 40,
                )?;
                if prior.identity() == source.identity() {
                    existing = true;
                    break;
                }
            }
            if !existing {
                push(sources, source.clone_with_budget(b)?, b)?;
            }
        }
    }
    if !found {
        return Err(RegionError::Access(BindingAccessError::MissingSource));
    }
    for i in 1..sources.len() {
        let mut j = i;
        while j > 0 {
            b.charge(
                Resource::Work,
                (sources[j - 1].identity().source.0.len() + sources[j].identity().source.0.len())
                    as u64
                    + 40,
            )?;
            if sources[j - 1].identity() <= sources[j].identity() {
                break;
            }
            sources.swap(j - 1, j);
            j -= 1;
        }
    }
    let mut out = Vec::new();
    let mut declaration_order = 0u64;
    let mut owner_depths = Vec::new();
    for _ in maps.entries() {
        push(&mut owner_depths, 0u64, b)?;
    }
    let mut pending = Vec::new();
    push(
        &mut pending,
        (0usize, input.binding.tree.tree().bundle.root, 1u64),
        b,
    )?;
    while let Some((owner, id, depth)) = pending.pop() {
        b.charge(Resource::Nodes, 1)?;
        b.observe_depth(depth)?;
        let mapping = maps.entries().get(owner).ok_or(RegionError::Owner)?;
        let bundle = mapping.bundle();
        if id == bundle.root {
            owner_depths[owner] = owner_depths[owner].max(depth);
        }
        let node = bundle
            .nodes
            .get(usize::try_from(id.0).map_err(|_| RegionError::Owner)?)
            .ok_or(RegionError::Owner)?;
        let context = check::context(input.binding, bundle, b)?;
        b.charge(Resource::Work, context.nodes.len() as u64)?;
        let selection = context
            .nodes
            .iter()
            .find(|v| v.node == id)
            .ok_or(RegionError::Owner)?;
        let package = input
            .binding
            .profile
            .language(&selection.entry.alias, b)
            .map_err(|e| match e {
                crate::profile::ProfileError::Stopped(v) => v.into(),
                _ => RegionError::Owner,
            })?;
        let (fields, styles, rules): (&[FieldSpec], &[StyleRule], &[SelectionRule]) =
            match &selection.shape {
                ShapeSelection::Form { index } => {
                    let v = package
                        .forms
                        .get(*index as usize)
                        .ok_or(RegionError::Owner)?;
                    (&v.fields, &v.styles, &v.selection_rules)
                }
                ShapeSelection::Leaf { index } => {
                    let v = package
                        .leaves
                        .get(*index as usize)
                        .ok_or(RegionError::Owner)?;
                    (&[], &v.styles, &v.selection_rules)
                }
                ShapeSelection::Dynamic { shape, .. } => {
                    (&shape.fields, &shape.styles, &shape.selection_rules)
                }
                _ => (&[], &[], &[]),
            };
        let mapped = mapping.mapped(id).map_err(|_| RegionError::Owner)?.0;
        let mut emit = |part,
                        at: &Span,
                        selector: Option<&StyleSelector>,
                        extra: &[PresentationClass],
                        d,
                        projection_owner: usize,
                        b: &mut Budget|
         -> Result<(), RegionError> {
            let order = declaration_order;
            declaration_order = declaration_order
                .checked_add(1)
                .ok_or_else(|| b.stop(nepl3_core::budget::StopReason::NodeLimit))?;
            let active = b
                .current_depth()
                .checked_add(d)
                .ok_or_else(|| b.stop(nepl3_core::budget::StopReason::DepthLimit))?;
            let projected = b.with_depth_at_least(active, |b| {
                super::mapping::project(
                    at,
                    &request.source,
                    &maps,
                    &maps.entries()[projection_owner].bundle().source_maps,
                    b,
                )
            })?;
            for (projected, quality) in projected {
                let (priority, classes) = presentation(selector, styles, rules, extra, b)?;
                let path = match &part {
                    RegionPart::View { path, .. } => path.len(),
                    _ => 0,
                };
                b.charge(Resource::Work, path as u64 + 1)?;
                b.charge(
                    Resource::AllocationUnits,
                    (path * core::mem::size_of::<ViewStep>()) as u64,
                )?;
                add(
                    &mut out,
                    SourceRegion {
                        target: RegionTarget {
                            bundle: owner as u64,
                            node: Some(mapped),
                            part: part.clone(),
                        },
                        span: projected,
                        logical_span: span(at, b)?,
                        mapping: quality,
                        priority,
                        depth: d,
                        declaration_order: order,
                        classes,
                    },
                    request,
                    b,
                )?;
            }
            Ok(())
        };
        if let Some(at) = &node.cover {
            let part = if matches!(selection.shape, ShapeSelection::Recovery) {
                RegionPart::Recovery
            } else {
                RegionPart::Node
            };
            emit(
                part,
                at,
                Some(&StyleSelector::SelfValue),
                &[],
                depth,
                owner,
                b,
            )?;
        }
        if let Some(at) = &node.head {
            emit(
                RegionPart::Head,
                at,
                Some(&StyleSelector::Head),
                &[],
                depth,
                owner,
                b,
            )?;
        }
        if let Some(token) = node.token {
            let token = bundle
                .tokens
                .get(token.0 as usize)
                .ok_or(RegionError::Owner)?;
            views(
                &token.views,
                depth,
                &mut |p, at, s, c, d, b| emit(p, at, s, c, d, owner, b),
                b,
            )?;
        }
        if let Some(batches) = input.facts {
            for (batch_index, batch) in batches.iter().enumerate() {
                b.charge(Resource::Work, 1)?;
                if batch.node != Some(id) {
                    continue;
                }
                let batch_owner = crate::tree::path(
                    &input.binding.tree.tree().bundle,
                    &batch.path,
                    input.binding.profile.registry(),
                    b,
                )
                .map_err(check::tree_error)?;
                if !core::ptr::eq(bundle, batch_owner) {
                    continue;
                }
                for (fact_index, fact) in batch.facts.iter().enumerate() {
                    b.charge(Resource::Work, 1)?;
                    match fact {
                        ReaderFact::Capture { name, span } => {
                            b.charge(Resource::Work, name.len() as u64)?;
                            b.charge(Resource::AllocationUnits, name.len() as u64)?;
                            emit(
                                RegionPart::Capture {
                                    batch: batch_index as u64,
                                    fact: fact_index as u64,
                                },
                                span,
                                Some(&StyleSelector::Capture(name.clone())),
                                &[],
                                depth + 1,
                                owner,
                                b,
                            )?;
                        }
                        ReaderFact::Presentation { class, span } => emit(
                            RegionPart::Presentation {
                                batch: batch_index as u64,
                                fact: fact_index as u64,
                            },
                            span,
                            None,
                            core::slice::from_ref(class),
                            depth + 1,
                            owner,
                            b,
                        )?,
                        ReaderFact::Relation { .. } => {}
                    }
                }
            }
        }
        for (field_index, field) in node.fields.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            let selector = if let Some(field) = fields.get(field_index) {
                b.charge(Resource::Work, field.name.len() as u64)?;
                b.charge(Resource::AllocationUnits, field.name.len() as u64)?;
                Some(StyleSelector::Field(field.name.clone()))
            } else {
                None
            };
            let mut child = |child_owner: usize,
                             child: NodeRef,
                             element,
                             b: &mut Budget|
             -> Result<(), RegionError> {
                let bundle = maps.entries()[child_owner].bundle();
                let child = bundle
                    .nodes
                    .get(child.0 as usize)
                    .ok_or(RegionError::Owner)?;
                if let Some(at) = &child.cover {
                    emit(
                        RegionPart::Field {
                            field: field_index as u64,
                            element,
                        },
                        at,
                        selector.as_ref(),
                        &[],
                        depth,
                        child_owner,
                        b,
                    )?;
                }
                Ok(())
            };
            match field {
                FieldValue::Child(id) => child(owner, *id, None, b)?,
                FieldValue::Children(ids) => {
                    for (i, id) in ids.iter().enumerate() {
                        child(owner, *id, Some(i as u64), b)?;
                    }
                }
                FieldValue::Foreign(foreign) => child(
                    bundle_index(&maps, &foreign.bundle, b)?,
                    foreign.root,
                    None,
                    b,
                )?,
                FieldValue::Atom(_) => {}
            }
        }
        // Field-order DFS controls declaration order; no arena-index tie break.
        for field in node.fields.iter().rev() {
            b.charge(Resource::Work, 1)?;
            match field {
                FieldValue::Child(id) => push(&mut pending, (owner, *id, depth + 1), b)?,
                FieldValue::Children(ids) => {
                    for id in ids.iter().rev() {
                        b.charge(Resource::Work, 1)?;
                        push(&mut pending, (owner, *id, depth + 1), b)?;
                    }
                }
                FieldValue::Foreign(foreign) => {
                    let index = bundle_index(&maps, &foreign.bundle, b)?;
                    push(&mut pending, (index, foreign.root, depth + 1), b)?;
                }
                FieldValue::Atom(_) => {}
            }
        }
    }
    if let Some(batches) = input.facts {
        for (batch_index, batch) in batches.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if batch.node.is_some() {
                continue;
            }
            let owner = crate::tree::path(
                &input.binding.tree.tree().bundle,
                &batch.path,
                input.binding.profile.registry(),
                b,
            )
            .map_err(check::tree_error)?;
            let index = bundle_index(&maps, owner, b)?;
            let depth = owner_depths[index];
            for (fact_index, fact) in batch.facts.iter().enumerate() {
                b.charge(Resource::Work, 1)?;
                let (part, at, extra) = match fact {
                    ReaderFact::Capture { span, .. } => (
                        RegionPart::Capture {
                            batch: batch_index as u64,
                            fact: fact_index as u64,
                        },
                        span,
                        &[][..],
                    ),
                    ReaderFact::Presentation { class, span } => (
                        RegionPart::Presentation {
                            batch: batch_index as u64,
                            fact: fact_index as u64,
                        },
                        span,
                        core::slice::from_ref(class),
                    ),
                    ReaderFact::Relation { .. } => continue,
                };
                let order = declaration_order;
                declaration_order = declaration_order
                    .checked_add(1)
                    .ok_or_else(|| b.stop(nepl3_core::budget::StopReason::NodeLimit))?;
                let active = b
                    .current_depth()
                    .checked_add(depth)
                    .ok_or_else(|| b.stop(nepl3_core::budget::StopReason::DepthLimit))?;
                let projected = b.with_depth_at_least(active, |b| {
                    super::mapping::project(at, &request.source, &maps, &owner.source_maps, b)
                })?;
                for (span, mapping) in projected {
                    let (_, classes) = presentation(None, &[], &[], extra, b)?;
                    add(
                        &mut out,
                        SourceRegion {
                            target: RegionTarget {
                                bundle: index as u64,
                                node: None,
                                part: part.clone(),
                            },
                            span,
                            logical_span: super::span(at, b)?,
                            mapping,
                            priority: 0,
                            depth,
                            declaration_order: order,
                            classes,
                        },
                        request,
                        b,
                    )?;
                }
            }
        }
    }
    let mut selected: Option<usize> = None;
    for (index, region) in out.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if region.span.start() <= request.offset
            && request.offset < region.span.end()
            && selected.is_none_or(|i| rank(region) < rank(&out[i]))
        {
            selected = Some(index);
        }
    }
    Ok(RegionOutcome::Complete {
        selection: selected.map(|v| v as u64),
        regions: out,
    })
}
fn rank(v: &SourceRegion) -> (u64, core::cmp::Reverse<u64>, core::cmp::Reverse<u64>, u64) {
    (
        v.span.end() - v.span.start(),
        core::cmp::Reverse(v.priority),
        core::cmp::Reverse(v.depth),
        v.declaration_order,
    )
}
fn bundle_index(
    map: &BundleMappings<'_>,
    bundle: &nepl3_core::syntax::SyntaxBundle,
    b: &mut Budget,
) -> Result<usize, RegionError> {
    for (i, entry) in map.entries().iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if core::ptr::eq(entry.bundle(), bundle) {
            return Ok(i);
        }
    }
    Err(RegionError::Owner)
}
fn presentation(
    selector: Option<&StyleSelector>,
    styles: &[StyleRule],
    rules: &[SelectionRule],
    extra: &[PresentationClass],
    b: &mut Budget,
) -> Result<(u64, Vec<PresentationClass>), RegionError> {
    let mut classes = Vec::new();
    let mut priority = 0;
    let size = |s: &StyleSelector| match s {
        StyleSelector::Field(v) | StyleSelector::Capture(v) => v.len(),
        _ => 0,
    };
    for rule in rules {
        b.charge(
            Resource::Work,
            (selector.map_or(0, size) + size(&rule.selector)) as u64 + 1,
        )?;
        if selector == Some(&rule.selector) {
            priority = rule.priority;
        }
    }
    for class in styles.iter().map(|rule| (&rule.selector, &rule.class)) {
        b.charge(
            Resource::Work,
            (selector.map_or(0, size) + size(class.0)) as u64 + 1,
        )?;
        if selector == Some(class.0) {
            copy_class(&mut classes, class.1, b)?;
        }
    }
    for class in extra {
        copy_class(&mut classes, class, b)?;
    }
    Ok((priority, classes))
}
fn copy_class(
    out: &mut Vec<PresentationClass>,
    class: &PresentationClass,
    b: &mut Budget,
) -> Result<(), RegionError> {
    let n = (class.schema.package.len() + class.name.len()) as u64;
    b.charge(Resource::Work, n + 40)?;
    b.charge(Resource::AllocationUnits, n)?;
    push(out, class.clone(), b)
}
fn add(
    out: &mut Vec<SourceRegion>,
    v: SourceRegion,
    request: &RegionRequest,
    b: &mut Budget,
) -> Result<(), RegionError> {
    let id = v.span.snapshot_ref();
    b.charge(
        Resource::Work,
        (id.source.0.len() + request.source.source_id.0.len()) as u64 + 40,
    )?;
    if id.source != request.source.source_id
        || id.revision != request.source.revision
        || id.digest != request.source.digest
    {
        return Ok(());
    }
    // Shared DAG nodes retain first declaration order and deepest actual path.
    for prior in out.iter_mut() {
        let path = |v: &RegionPart| match v {
            RegionPart::View { path, .. } => path.len(),
            _ => 0,
        };
        b.charge(
            Resource::Work,
            (path(&prior.target.part) + path(&v.target.part)) as u64 + 6,
        )?;
        if prior.target == v.target
            && prior.span.start() == v.span.start()
            && prior.span.end() == v.span.end()
            && prior.mapping == v.mapping
        {
            prior.depth = prior.depth.max(v.depth);
            return Ok(());
        }
    }
    push(out, v, b)
}
fn views(
    view: &ViewBundle,
    base: u64,
    emit: &mut impl FnMut(
        RegionPart,
        &Span,
        Option<&StyleSelector>,
        &[PresentationClass],
        u64,
        &mut Budget,
    ) -> Result<(), RegionError>,
    b: &mut Budget,
) -> Result<(), RegionError> {
    let mut pending = Vec::new();
    for (root, id) in view.roots.iter().enumerate().rev() {
        b.charge(Resource::Work, 1)?;
        push(&mut pending, (root as u64, *id, Vec::<ViewStep>::new()), b)?;
    }
    while let Some((root, ViewRef(id), path)) = pending.pop() {
        b.charge(Resource::Nodes, 1)?;
        b.observe_depth(base + path.len() as u64 + 1)?;
        let element = view.elements.get(id as usize).ok_or(RegionError::Owner)?;
        b.charge(Resource::Work, path.len() as u64)?;
        b.charge(
            Resource::AllocationUnits,
            (path.len() * core::mem::size_of::<ViewStep>()) as u64,
        )?;
        emit(
            RegionPart::View {
                root,
                path: path.clone(),
            },
            &element.span,
            None,
            &element.roles,
            base + path.len() as u64 + 1,
            b,
        )?;
        for (field_index, field) in element.fields.iter().enumerate().rev() {
            for (child_index, child) in field.children.iter().enumerate().rev() {
                b.charge(Resource::Work, path.len() as u64 + 1)?;
                b.charge(
                    Resource::AllocationUnits,
                    (path.len() * core::mem::size_of::<ViewStep>()) as u64,
                )?;
                let mut next = path.clone();
                push(
                    &mut next,
                    ViewStep {
                        field: field_index as u64,
                        child: child_index as u64,
                    },
                    b,
                )?;
                push(&mut pending, (root, *child, next), b)?;
            }
        }
    }
    Ok(())
}
