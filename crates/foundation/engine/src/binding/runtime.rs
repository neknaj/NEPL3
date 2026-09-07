use super::*;
mod custom;
#[derive(Clone, Copy, Eq, PartialEq)]
struct Target {
    bundle: usize,
    node: NodeRef,
}
#[derive(Clone, Copy)]
enum Effect {
    Root,
    Visit,
    Import,
    Propagate,
    Sequential,
}
#[derive(Clone, Copy, Default)]
struct Phase {
    header: bool,
    group: Option<ScopeId>,
}
enum Action {
    Plan {
        id: BindingId,
        depth: u64,
    },
    Restore(StageId),
    Child {
        index: usize,
        effect: Effect,
    },
    Declaration {
        target: Target,
        effect: Effect,
        phase: Phase,
    },
}
struct Frame {
    target: Target,
    stage: StageId,
    actions: Vec<Action>,
    exports: Vec<EntityId>,
    effect: Effect,
    phase: Phase,
}
struct ExportHeader {
    group: ScopeId,
    target: Target,
    binding: BindingId,
    entity: EntityId,
}
struct NamedOccurrence {
    namespace_stage: StageId,
    namespace: NamespaceRef,
    name: String,
    role: OccurrenceRole,
    span: Span,
    origin: OriginId,
    resolution: ReferenceResolution,
}
struct Layout<'a> {
    node: &'a SyntaxNode,
    package: &'a LanguagePackage,
    fields: &'a [FieldSpec],
    binding: Option<BindingId>,
    list: bool,
}
impl<'a, 'p> Machine<'a, 'p> {
    fn layout(&self, target: Target, budget: &mut Budget) -> Result<Layout<'a>, BindingError> {
        let local = self
            .bundles
            .get(target.bundle)
            .ok_or(BindingError::Target)?;
        let node = local
            .bundle
            .node(target.node)
            .map_err(crate::tree::TreeError::from)?;
        let mut selected = None;
        for value in local.selections {
            budget.charge(Resource::Work, 1)?;
            if value.node == target.node {
                selected = Some(value);
                break;
            }
        }
        let selected = selected.ok_or(BindingError::Target)?;
        let package = self.profile.language(&selected.entry.alias, budget)?;
        let (fields, binding, list) = match &selected.shape {
            ShapeSelection::Form { index } => {
                let value = package
                    .forms
                    .get(*index as usize)
                    .ok_or(BindingError::Target)?;
                (value.fields.as_slice(), Some(value.binding), false)
            }
            ShapeSelection::Leaf { index } => {
                let value = package
                    .leaves
                    .get(*index as usize)
                    .ok_or(BindingError::Target)?;
                (&[][..], Some(value.binding), false)
            }
            ShapeSelection::Dynamic { shape, .. } => {
                (shape.fields.as_slice(), Some(shape.binding), false)
            }
            ShapeSelection::Builtin { .. } => (&[][..], None, false),
            ShapeSelection::List { cons, .. } => (&[][..], None, *cons),
            ShapeSelection::Recovery => return Err(BindingError::RecoveredTree),
        };
        Ok(Layout {
            node,
            package,
            fields,
            binding,
            list,
        })
    }
    fn frame(
        &self,
        target: Target,
        stage: StageId,
        effect: Effect,
        phase: Phase,
        budget: &mut Budget,
    ) -> Result<Frame, BindingError> {
        let layout = self.layout(target, budget)?;
        let mut actions = Vec::new();
        if let Some(id) = layout.binding {
            push(&mut actions, Action::Plan { id, depth: 1 }, budget)?;
        } else if layout.list {
            push(
                &mut actions,
                Action::Child {
                    index: 1,
                    effect: Effect::Propagate,
                },
                budget,
            )?;
            push(
                &mut actions,
                Action::Child {
                    index: 0,
                    effect: Effect::Propagate,
                },
                budget,
            )?;
        }
        Ok(Frame {
            target,
            stage,
            actions,
            exports: Vec::new(),
            effect,
            phase,
        })
    }
    fn child_frame(
        &self,
        parent: &Frame,
        target: Target,
        effect: Effect,
        phase: Phase,
        budget: &mut Budget,
    ) -> Result<Frame, BindingError> {
        let stage = if target.bundle == parent.target.bundle {
            parent.stage
        } else {
            self.bundles
                .get(target.bundle)
                .ok_or(BindingError::Target)?
                .root
        };
        self.frame(target, stage, effect, phase, budget)
    }
    fn child(
        &self,
        target: Target,
        index: usize,
        budget: &mut Budget,
    ) -> Result<Target, BindingError> {
        budget.charge(Resource::Work, 1)?;
        let local = self
            .bundles
            .get(target.bundle)
            .ok_or(BindingError::Target)?;
        let node = local
            .bundle
            .node(target.node)
            .map_err(crate::tree::TreeError::from)?;
        match node.fields.get(index).ok_or(BindingError::Target)? {
            FieldValue::Child(node) => Ok(Target {
                bundle: target.bundle,
                node: *node,
            }),
            FieldValue::Foreign(value) => {
                for (index, local) in self.bundles.iter().enumerate() {
                    budget.charge(Resource::Work, 1)?;
                    if core::ptr::eq(local.bundle, &value.bundle) {
                        return Ok(Target {
                            bundle: index,
                            node: value.root,
                        });
                    }
                }
                Err(BindingError::Target)
            }
            _ => Err(BindingError::Target),
        }
    }
    fn field(layout: &Layout<'_>, name: &str, budget: &mut Budget) -> Result<usize, BindingError> {
        for (index, field) in layout.fields.iter().enumerate() {
            budget.charge(Resource::Work, name.len() as u64 + 1)?;
            if field.name == name {
                return Ok(index);
            }
        }
        Err(BindingError::Target)
    }
    fn name(
        &self,
        target: Target,
        layout: &Layout<'_>,
        selector: &NameSelector,
        budget: &mut Budget,
    ) -> Result<(String, Span, OriginId), BindingError> {
        let name_target = match selector {
            NameSelector::SelfValue => target,
            NameSelector::Field(name) => {
                self.child(target, Self::field(layout, name, budget)?, budget)?
            }
        };
        let local = self
            .bundles
            .get(name_target.bundle)
            .ok_or(BindingError::Target)?;
        let node = local
            .bundle
            .node(name_target.node)
            .map_err(crate::tree::TreeError::from)?;
        let token = local
            .bundle
            .tokens
            .get(node.token.ok_or(BindingError::Name)?.0 as usize)
            .ok_or(BindingError::Name)?;
        let NdfValue::Text(name) = &token.payload else {
            return Err(BindingError::Name);
        };
        if name.is_empty() {
            return Err(BindingError::Name);
        };
        Ok((
            text(name, budget)?,
            span(&token.head, budget)?,
            OriginId(local.origin_base + node.origin.0),
        ))
    }
    fn occurrence(
        &mut self,
        stage: StageId,
        value: NamedOccurrence,
        budget: &mut Budget,
    ) -> Result<OccurrenceId, BindingError> {
        let NamedOccurrence {
            namespace_stage,
            namespace,
            name,
            role,
            span,
            origin,
            resolution,
        } = value;
        let id = OccurrenceId(next_id(&self.facts()?.occurrences, |v| v.id.0, budget)?);
        let scope = self.stage(stage)?.scope;
        // Both vectors are prepared before either half of the association is published.
        budget.charge(Resource::Nodes, 1)?;
        budget.charge(
            Resource::AllocationUnits,
            (core::mem::size_of::<Occurrence>() + core::mem::size_of::<OccurrenceStage>()) as u64,
        )?;
        self.facts_mut()?.occurrences.push(Occurrence {
            id,
            scope,
            namespace,
            name,
            role,
            span,
            origin: Some(origin),
            resolution,
        });
        self.progress.occurrence_stages.push(OccurrenceStage {
            occurrence: id,
            stage,
            namespace_stage,
        });
        Ok(id)
    }
    fn named(
        &mut self,
        frame: &mut Frame,
        layout: &Layout<'_>,
        namespace: &str,
        selector: &NameSelector,
        role: OccurrenceRole,
        budget: &mut Budget,
    ) -> Result<(), BindingError> {
        let namespace = self.namespace(frame.target.bundle, layout.package, namespace, budget)?;
        let (name, selection, origin) = self.name(frame.target, layout, selector, budget)?;
        let global =
            self.facts()?.namespaces[namespace.0 as usize].policy == NamespacePolicy::Global;
        let namespace_stage = self.namespace_stage(frame.target.bundle, frame.stage, namespace)?;
        if role == OccurrenceRole::Reference {
            let resolution = self.resolve(namespace_stage, namespace, &name, budget)?;
            let undefined = matches!(resolution, ReferenceResolution::Unresolved(_));
            let ambiguous = matches!(resolution, ReferenceResolution::Ambiguous(_));
            let id = self.occurrence(
                frame.stage,
                NamedOccurrence {
                    namespace_stage,
                    namespace,
                    name: text(&name, budget)?,
                    role,
                    span: span(&selection, budget)?,
                    origin,
                    resolution,
                },
                budget,
            )?;
            if undefined {
                if self.facts()?.namespaces[namespace.0 as usize].policy == NamespacePolicy::Open {
                    push(&mut self.progress.open_inputs, id, budget)?;
                } else {
                    self.diagnostic("UndefinedName", namespace, &name, &selection, budget)?;
                }
            } else if ambiguous {
                self.diagnostic("AmbiguousName", namespace, &name, &selection, budget)?;
            }
        } else {
            if global && role == OccurrenceRole::Definition {
                self.unique_global(namespace_stage, namespace, &name, &selection, None, budget)?;
            }
            let id = EntityId(next_id(&self.facts()?.entities, |v| v.id.0, budget)?);
            let scope = self.stage(namespace_stage)?.scope;
            let definition = layout.node.cover.as_ref().unwrap_or(&selection);
            let entity = Entity {
                id,
                scope,
                namespace,
                name: text(&name, budget)?,
                definition: Some(span(definition, budget)?),
                selection: Some(span(&selection, budget)?),
                origin: Some(origin),
            };
            budget.charge(Resource::Nodes, 1)?;
            push(&mut self.facts_mut()?.entities, entity, budget)?;
            if role == OccurrenceRole::Definition {
                let next = self.introduce(namespace_stage, id, budget)?;
                if global {
                    self.bundles[frame.target.bundle].root = next;
                } else {
                    frame.stage = next;
                }
            } else {
                push(&mut frame.exports, id, budget)?;
            }
            self.occurrence(
                frame.stage,
                NamedOccurrence {
                    namespace_stage: self.namespace_stage(
                        frame.target.bundle,
                        frame.stage,
                        namespace,
                    )?,
                    namespace,
                    name,
                    role,
                    span: selection,
                    origin,
                    resolution: ReferenceResolution::Resolved(id),
                },
                budget,
            )?;
        }
        Ok(())
    }
    fn declarations(
        &self,
        mut target: Target,
        budget: &mut Budget,
    ) -> Result<Vec<Target>, BindingError> {
        let mut values = Vec::new();
        loop {
            let local = self
                .bundles
                .get(target.bundle)
                .ok_or(BindingError::Target)?;
            let mut cons = None;
            for selection in local.selections {
                budget.charge(Resource::Work, 1)?;
                if selection.node == target.node {
                    if let ShapeSelection::List { cons: value, .. } = selection.shape {
                        cons = Some(value);
                    }
                    break;
                }
            }
            match cons {
                Some(false) => return Ok(values),
                Some(true) => {
                    push(&mut values, self.child(target, 0, budget)?, budget)?;
                    target = self.child(target, 1, budget)?;
                }
                None => return Err(BindingError::Target),
            }
        }
    }
    pub(super) fn run(
        &mut self,
        root: usize,
        tree: &'a crate::recovery::ParseTree,
        host: &mut Option<&mut dyn BindingHost>,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), BindingError> {
        let local = self.bundles.get(root).ok_or(BindingError::Target)?;
        let frame = self.frame(
            Target {
                bundle: root,
                node: local.bundle.root,
            },
            local.root,
            Effect::Root,
            Phase::default(),
            budget,
        )?;
        let mut headers = Vec::<ExportHeader>::new();
        let mut frames = Vec::new();
        push(&mut frames, frame, budget)?;
        let depth_base = budget.current_depth();
        while let Some(mut frame) = frames.pop() {
            let action = frame.actions.pop();
            let plan_depth = match &action {
                Some(Action::Plan { depth, .. }) => *depth,
                _ => 0,
            };
            let depth = depth_base
                .saturating_add(frames.len() as u64 + 1)
                .saturating_add(plan_depth);
            budget.with_depth_at_least(depth, |budget| -> Result<(), BindingError> {
                budget.charge(Resource::Work, 1)?;
                let Some(action) = action else {
                    if let Some(parent) = frames.last_mut() {
                        match frame.effect {
                            Effect::Visit => {}
                            Effect::Propagate => {
                                for entity in frame.exports {
                                    push(&mut parent.exports, entity, budget)?;
                                }
                            }
                            Effect::Import | Effect::Sequential => {
                                if matches!(frame.effect, Effect::Sequential) {
                                    parent.stage = self.scope(Some(parent.stage), None, budget)?;
                                }
                                for entity in frame.exports {
                                    let value = self.entity(entity, budget)?;
                                    let target_root =
                                        self.stage(self.bundles[parent.target.bundle].root)?.scope;
                                    if self.facts()?.namespaces[value.namespace.0 as usize].root
                                        != target_root
                                    {
                                        return Err(BindingError::NamespaceBoundary);
                                    };
                                    let namespace = value.namespace;
                                    let global = self.facts()?.namespaces[namespace.0 as usize]
                                        .policy
                                        == NamespacePolicy::Global;
                                    let current = self.namespace_stage(
                                        parent.target.bundle,
                                        parent.stage,
                                        namespace,
                                    )?;
                                    let from = value.scope;
                                    if global {
                                        let name = text(&value.name, budget)?;
                                        let selection = span(
                                            value.selection.as_ref().ok_or(BindingError::Name)?,
                                            budget,
                                        )?;
                                        self.unique_global(
                                            current,
                                            namespace,
                                            &name,
                                            &selection,
                                            Some(entity),
                                            budget,
                                        )?;
                                    }
                                    let edge = ScopeEdge::Export {
                                        from,
                                        to: self.stage(current)?.scope,
                                        entity,
                                    };
                                    push(&mut self.facts_mut()?.edges, edge, budget)?;
                                    let next = self.introduce(current, entity, budget)?;
                                    if global {
                                        self.bundles[parent.target.bundle].root = next;
                                    } else {
                                        parent.stage = next;
                                    }
                                }
                            }
                            Effect::Root => return Err(BindingError::Target),
                        }
                    } else {
                        self.progress.exports = frame.exports;
                    }
                    return Ok(());
                };
                match action {
                    Action::Restore(stage) => frame.stage = stage,
                    Action::Child { index, effect } => {
                        let target = self.child(frame.target, index, budget)?;
                        let child =
                            self.child_frame(&frame, target, effect, frame.phase, budget)?;
                        push(&mut frames, frame, budget)?;
                        push(&mut frames, child, budget)?;
                        return Ok(());
                    }
                    Action::Declaration {
                        target,
                        effect,
                        phase,
                    } => {
                        let child = self.child_frame(&frame, target, effect, phase, budget)?;
                        push(&mut frames, frame, budget)?;
                        push(&mut frames, child, budget)?;
                        return Ok(());
                    }
                    Action::Plan { id, depth } => {
                        let layout = self.layout(frame.target, budget)?;
                        let binding = layout
                            .package
                            .bindings
                            .get(id.0 as usize)
                            .ok_or(BindingError::Target)?;
                        match binding {
                            Binding::None => {}
                            Binding::Group(children) | Binding::Scope(children) => {
                                if matches!(binding, Binding::Scope(_)) {
                                    let previous = frame.stage;
                                    frame.stage = self.scope(
                                        Some(previous),
                                        Some(OriginId(
                                            self.bundles[frame.target.bundle].origin_base
                                                + layout.node.origin.0,
                                        )),
                                        budget,
                                    )?;
                                    push(&mut frame.actions, Action::Restore(previous), budget)?;
                                }
                                for id in children.iter().rev() {
                                    push(
                                        &mut frame.actions,
                                        Action::Plan {
                                            id: *id,
                                            depth: depth + 1,
                                        },
                                        budget,
                                    )?;
                                }
                            }
                            Binding::Bind { .. } | Binding::Reference { .. }
                                if frame.phase.header => {}
                            Binding::Bind { namespace, name } => self.named(
                                &mut frame,
                                &layout,
                                namespace,
                                name,
                                OccurrenceRole::Definition,
                                budget,
                            )?,
                            Binding::Reference { namespace, name } => self.named(
                                &mut frame,
                                &layout,
                                namespace,
                                name,
                                OccurrenceRole::Reference,
                                budget,
                            )?,
                            Binding::Export { namespace, name } => {
                                let mut saved = None;
                                for value in &headers {
                                    budget.charge(Resource::Work, 2)?;
                                    if Some(value.group) == frame.phase.group
                                        && value.target == frame.target
                                        && value.binding == id
                                    {
                                        saved = Some(value.entity);
                                        break;
                                    }
                                }
                                if let Some(entity) = saved {
                                    push(&mut frame.exports, entity, budget)?;
                                } else {
                                    self.named(
                                        &mut frame,
                                        &layout,
                                        namespace,
                                        name,
                                        OccurrenceRole::Export,
                                        budget,
                                    )?;
                                    if frame.phase.header {
                                        let entity =
                                            *frame.exports.last().ok_or(BindingError::Target)?;
                                        push(
                                            &mut headers,
                                            ExportHeader {
                                                group: frame
                                                    .phase
                                                    .group
                                                    .ok_or(BindingError::Target)?,
                                                target: frame.target,
                                                binding: id,
                                                entity,
                                            },
                                            budget,
                                        )?;
                                    }
                                }
                            }
                            Binding::Visit(_) | Binding::Import(_) if frame.phase.header => {}
                            Binding::Visit(name)
                            | Binding::Import(name)
                            | Binding::Propagate(name) => {
                                let index = Self::field(&layout, name, budget)?;
                                let effect = match binding {
                                    Binding::Import(_) => Effect::Import,
                                    Binding::Propagate(_) => Effect::Propagate,
                                    _ => Effect::Visit,
                                };
                                push(&mut frame.actions, Action::Child { index, effect }, budget)?;
                            }
                            Binding::Custom(_) if frame.phase.header => {
                                return Err(BindingError::UnsupportedPlan);
                            }
                            Binding::Custom(operation) => {
                                self.custom(&mut frame, tree, operation, host, budget, admission)?
                            }
                            Binding::Sequential { .. } | Binding::Recursive { .. }
                                if frame.phase.header => {}
                            Binding::Sequential { declarations, body }
                            | Binding::Recursive { declarations, body } => {
                                let list = self.child(
                                    frame.target,
                                    Self::field(&layout, declarations, budget)?,
                                    budget,
                                )?;
                                let declarations = self.declarations(list, budget)?;
                                let body = self.child(
                                    frame.target,
                                    Self::field(&layout, body, budget)?,
                                    budget,
                                )?;
                                let previous = frame.stage;
                                push(&mut frame.actions, Action::Restore(previous), budget)?;
                                push(
                                    &mut frame.actions,
                                    Action::Declaration {
                                        target: body,
                                        effect: Effect::Visit,
                                        phase: Phase::default(),
                                    },
                                    budget,
                                )?;
                                if matches!(binding, Binding::Recursive { .. }) {
                                    frame.stage = self.scope(Some(previous), None, budget)?;
                                    let group = Some(self.stage(frame.stage)?.scope);
                                    for target in declarations.iter().rev() {
                                        push(
                                            &mut frame.actions,
                                            Action::Declaration {
                                                target: *target,
                                                effect: Effect::Visit,
                                                phase: Phase {
                                                    header: false,
                                                    group,
                                                },
                                            },
                                            budget,
                                        )?;
                                    }
                                    for target in declarations.iter().rev() {
                                        push(
                                            &mut frame.actions,
                                            Action::Declaration {
                                                target: *target,
                                                effect: Effect::Import,
                                                phase: Phase {
                                                    header: true,
                                                    group,
                                                },
                                            },
                                            budget,
                                        )?;
                                    }
                                } else {
                                    for target in declarations.iter().rev() {
                                        push(
                                            &mut frame.actions,
                                            Action::Declaration {
                                                target: *target,
                                                effect: Effect::Sequential,
                                                phase: Phase::default(),
                                            },
                                            budget,
                                        )?;
                                    }
                                }
                            }
                        }
                    }
                }
                push(&mut frames, frame, budget)?;
                Ok(())
            })?;
        }
        Ok(())
    }
}
