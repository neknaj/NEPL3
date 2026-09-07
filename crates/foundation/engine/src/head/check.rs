//! Receiving-side projection checks require no original full snapshot. They
//! prove internal consistency, not the issuer's identity or reader history.
mod report;
use super::*;
use crate::profile::{ProfileError, ResolvedParseProfile};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeShape},
    value::{NdfScalar, NdfValue, SchemaRef},
};

fn profile_error(error: ProfileError) -> HeadError {
    match error {
        ProfileError::Stopped(reason) => HeadError::Stopped(reason),
        _ => HeadError::Context,
    }
}
fn package_error(error: crate::package::PackageError) -> HeadError {
    match error {
        crate::package::PackageError::Stopped(reason) => HeadError::Stopped(reason),
        _ => HeadError::Shape,
    }
}
fn push<T>(items: &mut Vec<T>, item: T, budget: &mut Budget) -> Result<(), HeadError> {
    budget.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    items.push(item);
    Ok(())
}
fn schema_eq(a: &SchemaRef, b: &SchemaRef, budget: &mut Budget) -> Result<bool, HeadError> {
    budget.charge(
        Resource::Work,
        (a.package.len() as u64)
            .saturating_add(b.package.len() as u64)
            .saturating_add(34),
    )?;
    Ok(a == b)
}
fn contains(a: &ProjectedSpan, b: &ProjectedSpan, budget: &mut Budget) -> Result<bool, HeadError> {
    Ok(a.same_snapshot(b, budget)? && a.start <= b.start && b.start <= b.end && b.end <= a.end)
}

impl HeadCall {
    /// Check a received call against the selected registry/profile and explicit
    /// windows. Snapshot digests and claimed covers remain issuer assertions;
    /// successful validation does not authorize provider execution.
    pub fn validate_projection(
        &self,
        profile: &ResolvedParseProfile<'_>,
        budget: &mut Budget,
    ) -> Result<(), HeadError> {
        budget.charge(Resource::Work, 1)?;
        budget.with_depth_at_least(self.depth_base, |budget| {
            self.check_projection(profile, budget)
        })
    }
    fn check_projection(
        &self,
        profile: &ResolvedParseProfile<'_>,
        budget: &mut Budget,
    ) -> Result<(), HeadError> {
        if self.identity.session_id.is_empty()
            || self.identity.profile_digest != profile.digest()
            || self.identity.execution_digest
                != profile
                    .execution_digest(&self.entry.alias, budget)
                    .map_err(profile_error)?
        {
            return Err(HeadError::Identity);
        }
        let checked = profile
            .validate_entry(&self.entry, budget)
            .map_err(profile_error)?;
        let registered = profile
            .head_provider(&self.entry.alias, &self.entry.category, budget)
            .map_err(profile_error)?
            .ok_or(HeadError::Signature)?;
        let operation = match self.request {
            HeadRequest::Shape => &registered.shape,
            HeadRequest::ChildContext { .. } => &registered.child_context,
        };
        budget.charge(
            Resource::Work,
            (operation.name.len() as u64).saturating_add(self.identity.operation.name.len() as u64),
        )?;
        if !schema_eq(&operation.schema, &self.identity.operation.schema, budget)?
            || operation.name != self.identity.operation.name
        {
            return Err(HeadError::Signature);
        }
        for (i, window) in self.windows().enumerate() {
            window.validate(budget)?;
            for previous in self.windows().take(i) {
                compatible(previous, window, budget)?;
            }
        }
        if !contains(&self.head.window.span, &self.head.token.head, budget)?
            || self.head.window.span.start != self.head.token.head.start
            || self.head.window.span.end != self.head.token.head.end
        {
            return Err(HeadError::Projection);
        }
        self.check_token(&self.head.token, profile.registry(), budget)?;
        if let HeadRequest::ChildContext {
            shape,
            index,
            completed,
        } = &self.request
        {
            checked
                .validate_head_shape(shape, budget)
                .map_err(package_error)?;
            if *index >= shape.fields.len() as u64 || *index != completed.roots.len() as u64 {
                return Err(HeadError::Shape);
            }
            self.check_graph(completed, profile.registry(), budget)?;
            for (i, root) in completed.roots.iter().enumerate() {
                let node = &completed.nodes[root.0 as usize]; // check_graph proved every index.
                if let Some(cover) = &node.cover {
                    if cover.same_snapshot(&self.head.token.head, budget)?
                        && cover.start < self.head.token.head.end
                    {
                        return Err(HeadError::Projection);
                    }
                    for previous in &completed.roots[..i] {
                        if let Some(prior) = &completed.nodes[previous.0 as usize].cover
                            && prior.same_snapshot(cover, budget)?
                            && prior.end > cover.start
                        {
                            return Err(HeadError::Projection);
                        }
                    }
                }
            }
            // Each declared window must be justified by explicit projected
            // geometry. No payload field can introduce lookup authority.
            for window in &completed.windows {
                let mut owned = false;
                for node in &completed.nodes {
                    for span in node
                        .head
                        .iter()
                        .chain(node.cover.iter())
                        .chain(node.token.iter().map(|v| &v.head))
                    {
                        if contains(span, &window.span, budget)? {
                            owned = true;
                            break;
                        }
                    }
                    if owned {
                        break;
                    }
                }
                if !owned {
                    return Err(HeadError::Projection);
                }
            }
        }
        Ok(())
    }
    fn check_token(
        &self,
        token: &ProjectedToken,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), HeadError> {
        self.slice(&token.head, budget)?;
        budget.charge(Resource::Work, token.kind.schema.package.len() as u64 + 34)?;
        registry.kind_name(&token.kind.schema, token.kind.local_kind)?;
        registry.validate(&TypeDescriptor::NdfValue, &token.payload, budget)?;
        Ok(())
    }
    fn check_graph(
        &self,
        syntax: &ProjectedSyntax,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), HeadError> {
        let mut visiting = Vec::new();
        let mut reached = Vec::new();
        for _ in &syntax.nodes {
            push(&mut visiting, false, budget)?;
            push(&mut reached, false, budget)?;
        }
        let mut stack = Vec::new();
        for root in syntax.roots.iter().rev() {
            push(&mut stack, (*root, false, 1u64), budget)?;
        }
        while let Some((id, exit, depth)) = stack.pop() {
            budget.charge(Resource::Work, 1)?;
            let index = usize::try_from(id.0).map_err(|_| HeadError::Projection)?;
            let node = syntax.nodes.get(index).ok_or(HeadError::Projection)?;
            if exit {
                visiting[index] = false;
                continue;
            }
            budget.observe_depth(depth)?;
            if visiting[index] {
                return Err(HeadError::Projection);
            }
            if !reached[index] {
                budget.charge(Resource::Nodes, 1)?;
            }
            // Payload validation is nested inside this graph node, including
            // when a shared node is reached along a deeper second path.
            let node_depth = budget
                .current_depth()
                .checked_add(depth)
                .ok_or(StopReason::DepthLimit)?;
            budget.with_depth_at_least(node_depth, |budget| {
                self.check_node(node, syntax, registry, budget)
            })?;
            reached[index] = true;
            visiting[index] = true;
            push(&mut stack, (id, true, depth), budget)?;
            let depth = depth.checked_add(1).ok_or(StopReason::DepthLimit)?;
            for field in node.fields.iter().rev() {
                match field {
                    ProjectedFieldValue::Child(id)
                    | ProjectedFieldValue::Foreign { root: id, .. } => {
                        push(&mut stack, (*id, false, depth), budget)?
                    }
                    ProjectedFieldValue::Children(ids) => {
                        for id in ids.iter().rev() {
                            push(&mut stack, (*id, false, depth), budget)?;
                        }
                    }
                    ProjectedFieldValue::Atom(_) => {}
                }
            }
        }
        budget.charge(Resource::Work, reached.len() as u64)?;
        if reached.iter().any(|v| !v) {
            return Err(HeadError::Projection);
        }
        Ok(())
    }
    fn check_node(
        &self,
        node: &ProjectedNode,
        syntax: &ProjectedSyntax,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), HeadError> {
        for span in node.head.iter().chain(node.cover.iter()) {
            self.slice(span, budget)?;
        }
        if let Some(head) = &node.head {
            let cover = node.cover.as_ref().ok_or(HeadError::Projection)?;
            if !contains(cover, head, budget)? {
                return Err(HeadError::Projection);
            }
        }
        if let Some(token) = &node.token {
            self.check_token(token, registry, budget)?;
            if let Some(head) = &node.head
                && (!contains(head, &token.head, budget)?
                    || head.start != token.head.start
                    || head.end != token.head.end)
            {
                return Err(HeadError::Projection);
            }
        }
        budget.charge(Resource::Work, node.schema.package.len() as u64 + 34)?;
        let descriptor = registry
            .descriptor(&node.schema)
            .ok_or(SchemaError::UnknownSchema)?;
        let mut record = None;
        for ty in &descriptor.types {
            budget.charge(
                Resource::Work,
                (ty.name.len() as u64)
                    .saturating_add(node.kind.len() as u64)
                    .saturating_add(1),
            )?;
            if ty.name == node.kind {
                record = Some(&ty.shape);
                break;
            }
        }
        let Some(TypeShape::Record { fields }) = record else {
            return Err(HeadError::Shape);
        };
        if fields.len() != node.fields.len() {
            return Err(HeadError::Shape);
        }
        let mut last = None;
        for (field, expected) in node.fields.iter().zip(fields) {
            budget.charge(Resource::Work, 1)?;
            match field {
                ProjectedFieldValue::Atom(value) => {
                    super::projection::charge_scalar(value, budget)?;
                    let value = match value {
                        NdfScalar::Unit => NdfValue::Unit,
                        NdfScalar::Bool(v) => NdfValue::Bool(*v),
                        NdfScalar::U64(v) => NdfValue::U64(*v),
                        NdfScalar::Integer(v) => NdfValue::Integer(v.clone()),
                        NdfScalar::Rational(v) => NdfValue::Rational(v.clone()),
                        NdfScalar::Text(v) => NdfValue::Text(v.clone()),
                        NdfScalar::Bytes(v) => NdfValue::Bytes(v.clone()),
                    };
                    registry.validate(&expected.ty, &value, budget)?;
                }
                ProjectedFieldValue::Child(id) if named(&expected.ty, "NodeRef", budget)? => {
                    child_geometry(node, *id, syntax, &mut last, budget)?;
                }
                ProjectedFieldValue::Children(ids) if matches!(&expected.ty, TypeDescriptor::List(inner) if named(inner, "NodeRef", budget)?) => {
                    for id in ids {
                        child_geometry(node, *id, syntax, &mut last, budget)?;
                    }
                }
                ProjectedFieldValue::Foreign {
                    schema,
                    category,
                    root,
                    ..
                } if named(&expected.ty, "ForeignSyntax", budget)? => {
                    let root = syntax
                        .nodes
                        .get(usize::try_from(root.0).map_err(|_| HeadError::Projection)?)
                        .ok_or(HeadError::Projection)?;
                    if category.is_empty() || !schema_eq(schema, &root.schema, budget)? {
                        return Err(HeadError::Projection);
                    }
                    // EnvironmentRef retains the original bundle's local ID.
                    // There is deliberately no environment table to resolve.
                }
                _ => return Err(HeadError::Shape),
            }
        }
        Ok(())
    }
}
fn child_geometry<'a>(
    parent: &ProjectedNode,
    id: ProjectedNodeRef,
    syntax: &'a ProjectedSyntax,
    last: &mut Option<&'a ProjectedSpan>,
    budget: &mut Budget,
) -> Result<(), HeadError> {
    let child = syntax
        .nodes
        .get(usize::try_from(id.0).map_err(|_| HeadError::Projection)?)
        .ok_or(HeadError::Projection)?;
    if let Some(cover) = &parent.cover {
        let child = child.cover.as_ref().ok_or(HeadError::Projection)?;
        if !contains(cover, child, budget)?
            || parent.head.as_ref().is_some_and(|v| v.end > child.start)
            || last.is_some_and(|v| v.end > child.start)
        {
            return Err(HeadError::Projection);
        }
        *last = Some(child);
    }
    Ok(())
}
fn named(ty: &TypeDescriptor, name: &str, budget: &mut Budget) -> Result<bool, HeadError> {
    if let TypeDescriptor::Named(ty) = ty {
        budget.charge(
            Resource::Work,
            (ty.package.len() as u64)
                .saturating_add(ty.name.len() as u64)
                .saturating_add(32),
        )?;
        Ok(ty.package == "nepl3.foundation" && ty.revision == 1 && ty.name == name)
    } else {
        Ok(false)
    }
}

pub(crate) fn compatible(
    a: &SourceWindow,
    b: &SourceWindow,
    budget: &mut Budget,
) -> Result<(), HeadError> {
    budget.charge(
        Resource::Work,
        (a.span.source.source_id.0.len() as u64)
            .saturating_add(b.span.source.source_id.0.len() as u64)
            .saturating_add(34),
    )?;
    if a.span.source.source_id != b.span.source.source_id
        || a.span.source.revision != b.span.source.revision
    {
        return Ok(());
    }
    if a.span.source.digest != b.span.source.digest {
        return Err(HeadError::Identity);
    }
    let start = a.span.start.max(b.span.start);
    let end = a.span.end.min(b.span.end);
    if start <= end {
        fn part(window: &SourceWindow, start: u64, end: u64) -> Result<&str, HeadError> {
            let start =
                usize::try_from(start - window.span.start).map_err(|_| HeadError::Projection)?;
            let end =
                usize::try_from(end - window.span.start).map_err(|_| HeadError::Projection)?;
            core::str::from_utf8(&window.bytes)
                .map_err(|_| HeadError::Projection)?
                .get(start..end)
                .ok_or(HeadError::Projection)
        }
        budget.charge(
            Resource::Work,
            (a.bytes.len() as u64)
                .saturating_add(b.bytes.len() as u64)
                .saturating_add(end - start),
        )?;
        if part(a, start, end)? != part(b, start, end)? {
            return Err(HeadError::Projection);
        }
    }
    Ok(())
}
