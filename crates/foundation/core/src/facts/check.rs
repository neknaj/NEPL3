use super::*;
use crate::{
    origin::{OriginGraph, SourceMap},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceStore},
};
/// Proves source/ID/type/namespace-root integrity. It does not run lexical name
/// resolution or prove that a claimed resolved entity is visible at an occurrence.
pub struct CheckedFactSet<'a> {
    value: &'a FactSet,
    registry: &'a SchemaRegistry,
}
impl CheckedFactSet<'_> {
    pub fn value(&self) -> &FactSet {
        self.value
    }
    /// Check alternate resolutions against this set's entity/namespace/source
    /// closure. This does not prove lexical visibility or permission to update.
    pub fn validate_resolutions<'r>(
        &self,
        resolutions: impl IntoIterator<Item = (OccurrenceId, &'r ReferenceResolution)>,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), FactError> {
        let view = View::new(self.value, None, budget, admission)?;
        for (id, resolution) in resolutions {
            let occurrence = view.occurrence(id, budget)?;
            view.resolution(occurrence, resolution, self.registry, budget)?;
        }
        Ok(())
    }
}
pub struct CheckedFactDelta<'a> {
    pub(super) base: &'a FactSet,
    pub(super) delta: &'a FactDelta,
    authority: &'a FactAuthority,
}
impl CheckedFactDelta<'_> {
    pub fn authority(&self) -> &FactAuthority {
        self.authority
    }
    pub fn base(&self) -> &FactSet {
        self.base
    }
    pub fn delta(&self) -> &FactDelta {
        self.delta
    }
}
struct View<'a> {
    base: &'a FactSet,
    scopes: Vec<&'a Scope>,
    entities: Vec<&'a Entity>,
    occurrences: Vec<&'a Occurrence>,
    relations: Vec<&'a Relation>,
    edges: Vec<&'a ScopeEdge>,
    origins: Vec<Origin>,
    sources: SourceStore,
}
fn find<'a, T>(
    values: &[&'a T],
    id: u64,
    key: impl Fn(&T) -> u64,
    budget: &mut Budget,
) -> Result<Option<&'a T>, FactError> {
    for value in values {
        budget.charge(Resource::Work, 1)?;
        if key(value) == id {
            return Ok(Some(value));
        }
    }
    Ok(None)
}
fn unique<T>(values: &[&T], key: impl Fn(&T) -> u64, budget: &mut Budget) -> Result<(), FactError> {
    for (i, value) in values.iter().enumerate() {
        if find(&values[..i], key(value), &key, budget)?.is_some() {
            return Err(FactError::DuplicateId);
        }
    }
    Ok(())
}
impl<'a> View<'a> {
    fn new(
        base: &'a FactSet,
        delta: Option<&'a FactDelta>,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Self, FactError> {
        let mut v = Self {
            base,
            scopes: Vec::new(),
            entities: Vec::new(),
            occurrences: Vec::new(),
            relations: Vec::new(),
            edges: Vec::new(),
            origins: Vec::new(),
            sources: SourceStore::default(),
        };
        for value in &base.scopes {
            push(&mut v.scopes, value, budget)?;
        }
        for value in &base.entities {
            push(&mut v.entities, value, budget)?;
        }
        for value in &base.occurrences {
            push(&mut v.occurrences, value, budget)?;
        }
        for value in &base.relations {
            push(&mut v.relations, value, budget)?;
        }
        for value in &base.edges {
            push(&mut v.edges, value, budget)?;
        }
        if let Some(d) = delta {
            for value in &d.scopes {
                push(&mut v.scopes, value, budget)?;
            }
            for value in &d.entities {
                push(&mut v.entities, value, budget)?;
            }
            for value in &d.occurrences {
                push(&mut v.occurrences, value, budget)?;
            }
            for value in &d.relations {
                push(&mut v.relations, value, budget)?;
            }
            for value in &d.edges {
                push(&mut v.edges, value, budget)?;
            }
        }
        for source in base
            .sources
            .iter()
            .chain(delta.into_iter().flat_map(|d| &d.sources))
        {
            admission.admit_existing(source, budget)?;
            if v.sources.get_ref(source.identity()).is_some() {
                return Err(FactError::DuplicateId);
            }
            v.sources
                .insert_with_budget(source.clone_with_budget(budget)?, budget)?;
        }
        for origin in base
            .origins
            .iter()
            .chain(delta.into_iter().flat_map(|d| &d.origins))
        {
            push(&mut v.origins, origin.clone_with_budget(budget)?, budget)?;
        }
        OriginGraph::validate_origins(&v.origins, &v.sources, budget)?;
        let mut mappings = Vec::new();
        for map in base
            .source_maps
            .iter()
            .chain(delta.into_iter().flat_map(|d| &d.source_maps))
        {
            push(&mut mappings, map.clone_with_budget(budget)?, budget)?;
        }
        SourceMap::validate_mappings(&mappings, &v.sources, budget)?;
        Ok(v)
    }
    fn scope(&self, id: ScopeId, budget: &mut Budget) -> Result<&Scope, FactError> {
        find(&self.scopes, id.0, |v| v.id.0, budget)?.ok_or(FactError::MissingScope)
    }
    fn entity(&self, id: EntityId, budget: &mut Budget) -> Result<&Entity, FactError> {
        find(&self.entities, id.0, |v| v.id.0, budget)?.ok_or(FactError::MissingEntity)
    }
    fn occurrence(&self, id: OccurrenceId, budget: &mut Budget) -> Result<&Occurrence, FactError> {
        find(&self.occurrences, id.0, |v| v.id.0, budget)?.ok_or(FactError::MissingOccurrence)
    }
    fn namespace(&self, id: NamespaceRef) -> Result<&FactNamespace, FactError> {
        usize::try_from(id.0)
            .ok()
            .and_then(|i| self.base.namespaces.get(i))
            .ok_or(FactError::MissingNamespace)
    }
    fn origin(&self, id: Option<OriginId>) -> Result<(), FactError> {
        if id.is_some_and(|v| v.0 >= self.origins.len() as u64) {
            return Err(FactError::MissingOrigin);
        }
        Ok(())
    }
    fn span(&self, span: &Span, budget: &mut Budget) -> Result<(), FactError> {
        budget.charge(
            Resource::Work,
            (self.sources.snapshots().len() as u64 + 1)
                .saturating_mul(span.snapshot_ref().source.0.len() as u64 + 1),
        )?;
        self.sources
            .get_ref(span.snapshot_ref())
            .ok_or(FactError::Span)?
            .slice(span)?;
        Ok(())
    }
    fn name(&self, name: &str, budget: &mut Budget) -> Result<(), FactError> {
        budget.charge(Resource::Work, name.len() as u64 + 1)?;
        if name.is_empty() {
            return Err(FactError::Name);
        }
        Ok(())
    }
    fn typed(
        &self,
        value: &TypedValue,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), FactError> {
        registry.validate_typed(value, budget)?;
        Ok(())
    }
    fn target(&self, target: &FactTarget, budget: &mut Budget) -> Result<(), FactError> {
        match target {
            FactTarget::Scope(v) => {
                self.scope(*v, budget)?;
            }
            FactTarget::Entity(v) => {
                self.entity(*v, budget)?;
            }
            FactTarget::Occurrence(v) => {
                self.occurrence(*v, budget)?;
            }
            FactTarget::Source(v) => self.span(v, budget)?,
        }
        Ok(())
    }
    fn resolution(
        &self,
        occurrence: &Occurrence,
        resolution: &ReferenceResolution,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), FactError> {
        let entity = |id, budget: &mut Budget| -> Result<(), FactError> {
            let e = self.entity(id, budget)?;
            budget.charge(
                Resource::Work,
                e.name.len().min(occurrence.name.len()) as u64 + 1,
            )?;
            let a = self.namespace(e.namespace)?;
            let b = self.namespace(occurrence.namespace)?;
            budget.charge(
                Resource::Work,
                (a.name.len() + a.schema.package.len()) as u64 + 41,
            )?;
            if a.schema != b.schema
                || a.name != b.name
                || a.policy != b.policy
                || e.name != occurrence.name
            {
                return Err(FactError::Resolution);
            }
            Ok(())
        };
        match resolution {
            ReferenceResolution::Resolved(id) => entity(*id, budget)?,
            ReferenceResolution::Unresolved(name) => {
                budget.charge(Resource::Work, name.len() as u64 + 1)?;
                if name != &occurrence.name {
                    return Err(FactError::Resolution);
                }
            }
            ReferenceResolution::Ambiguous(ids) => {
                if ids.len() < 2 {
                    return Err(FactError::Resolution);
                }
                for (i, id) in ids.iter().enumerate() {
                    budget.charge(Resource::Work, i as u64 + 1)?;
                    if ids[..i].contains(id) {
                        return Err(FactError::Resolution);
                    }
                    entity(*id, budget)?;
                }
            }
            ReferenceResolution::Deferred(values) => {
                if values.is_empty() {
                    return Err(FactError::Resolution);
                }
                for value in values {
                    self.typed(value, registry, budget)?;
                }
            }
        }
        Ok(())
    }
    fn descendant(
        &self,
        mut scope: ScopeId,
        root: ScopeId,
        budget: &mut Budget,
    ) -> Result<bool, FactError> {
        for depth in 0..=self.scopes.len() {
            budget.observe_depth(depth as u64 + 1)?;
            if scope == root {
                return Ok(true);
            }
            let Some(parent) = self.scope(scope, budget)?.parent else {
                return Ok(false);
            };
            scope = parent;
        }
        Err(FactError::Cycle)
    }
    fn validate(&self, registry: &SchemaRegistry, budget: &mut Budget) -> Result<(), FactError> {
        budget.charge(Resource::Work, self.base.analysis_id.len() as u64 + 1)?;
        if self.base.analysis_id.is_empty() {
            return Err(FactError::Analysis);
        }
        unique(&self.scopes, |v| v.id.0, budget)?;
        unique(&self.entities, |v| v.id.0, budget)?;
        unique(&self.occurrences, |v| v.id.0, budget)?;
        unique(&self.relations, |v| v.id.0, budget)?;
        for (i, namespace) in self.base.namespaces.iter().enumerate() {
            self.name(&namespace.name, budget)?;
            if registry.descriptor(&namespace.schema).is_none() {
                return Err(FactError::MissingNamespace);
            }
            self.scope(namespace.root, budget)?;
            for prior in &self.base.namespaces[..i] {
                budget.charge(
                    Resource::Work,
                    (namespace.name.len() + namespace.schema.package.len()) as u64 + 41,
                )?;
                if prior.schema == namespace.schema
                    && prior.name == namespace.name
                    && prior.root == namespace.root
                {
                    return Err(FactError::DuplicateId);
                }
            }
        }
        for scope in &self.scopes {
            budget.charge(Resource::Nodes, 1)?;
            self.origin(scope.origin)?;
            let mut current = scope.parent;
            let mut steps = 0;
            while let Some(id) = current {
                budget.observe_depth(steps + 1)?;
                if id == scope.id || steps > self.scopes.len() as u64 {
                    return Err(FactError::Cycle);
                }
                current = self.scope(id, budget)?.parent;
                steps += 1;
            }
        }
        for entity in &self.entities {
            budget.charge(Resource::Nodes, 1)?;
            self.name(&entity.name, budget)?;
            self.scope(entity.scope, budget)?;
            let ns = self.namespace(entity.namespace)?;
            if !self.descendant(entity.scope, ns.root, budget)? {
                return Err(FactError::MissingNamespace);
            }
            self.origin(entity.origin)?;
            if let Some(span) = &entity.definition {
                self.span(span, budget)?;
            }
            if let Some(span) = &entity.selection {
                self.span(span, budget)?;
                if !entity.definition.as_ref().is_some_and(|d| d.contains(span)) {
                    return Err(FactError::Span);
                }
            }
        }
        for occurrence in &self.occurrences {
            budget.charge(Resource::Nodes, 1)?;
            self.name(&occurrence.name, budget)?;
            self.scope(occurrence.scope, budget)?;
            let ns = self.namespace(occurrence.namespace)?;
            if !self.descendant(occurrence.scope, ns.root, budget)? {
                return Err(FactError::MissingNamespace);
            }
            self.span(&occurrence.span, budget)?;
            self.origin(occurrence.origin)?;
            self.resolution(occurrence, &occurrence.resolution, registry, budget)?;
        }
        for relation in &self.relations {
            budget.charge(Resource::Nodes, 1)?;
            self.target(&relation.source, budget)?;
            self.target(&relation.target, budget)?;
            self.typed(&relation.payload, registry, budget)?;
        }
        for edge in &self.edges {
            budget.charge(Resource::Work, 1)?;
            match edge {
                ScopeEdge::Import {
                    from,
                    to,
                    namespace,
                } => {
                    self.scope(*from, budget)?;
                    self.scope(*to, budget)?;
                    self.namespace(*namespace)?;
                }
                ScopeEdge::Export { from, to, entity } => {
                    self.scope(*from, budget)?;
                    self.scope(*to, budget)?;
                    if self.entity(*entity, budget)?.scope != *from {
                        return Err(FactError::Authority);
                    }
                }
            }
        }
        Ok(())
    }
}
impl FactSet {
    pub fn validate<'a>(
        &'a self,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<CheckedFactSet<'a>, FactError> {
        budget.charge(Resource::Work, 1)?;
        View::new(self, None, budget, admission)?.validate(registry, budget)?;
        Ok(CheckedFactSet {
            value: self,
            registry,
        })
    }
}
fn has<T: PartialEq>(values: &[T], value: &T, budget: &mut Budget) -> Result<bool, FactError> {
    budget.charge(Resource::Work, values.len() as u64 + 1)?;
    Ok(values.contains(value))
}
impl FactAuthority {
    /// Checks that a host's proposed grant refers to the declared existing
    /// analysis. This validates its shape and scope bounds, not its provenance:
    /// an operation receiver must still match the exact host-issued grant.
    pub fn validate(
        &self,
        base: &CheckedFactSet<'_>,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), FactError> {
        budget.charge(Resource::Work, self.analysis_id.len() as u64 + 1)?;
        if self.analysis_id != base.value.analysis_id {
            return Err(FactError::Analysis);
        }
        let view = View::new(base.value, None, budget, admission)?;
        view.scope(self.current_scope, budget)?;
        for (i, ns) in self.namespaces.iter().enumerate() {
            if has(&self.namespaces[..i], ns, budget)? {
                return Err(FactError::DuplicateId);
            }
            view.namespace(*ns)?;
        }
        for (i, scope) in self.writable_scopes.iter().enumerate() {
            if has(&self.writable_scopes[..i], scope, budget)?
                || !view.descendant(*scope, self.current_scope, budget)?
            {
                return Err(FactError::Authority);
            }
        }
        for (i, scope) in self.import_scopes.iter().enumerate() {
            if has(&self.import_scopes[..i], scope, budget)? {
                return Err(FactError::DuplicateId);
            }
            view.scope(*scope, budget)?;
        }
        for (i, id) in self.resolution_updates.iter().enumerate() {
            if has(&self.resolution_updates[..i], id, budget)? {
                return Err(FactError::DuplicateId);
            }
            let occurrence = view.occurrence(*id, budget)?;
            if !(occurrence.scope == self.current_scope
                || has(&self.writable_scopes, &occurrence.scope, budget)?)
                || !has(&self.namespaces, &occurrence.namespace, budget)?
            {
                return Err(FactError::Authority);
            }
        }
        for target in &self.relation_sources {
            view.target(target, budget)?;
        }
        for range in [
            self.reservation.scopes,
            self.reservation.entities,
            self.reservation.occurrences,
            self.reservation.relations,
        ] {
            budget.charge(Resource::Work, 1)?;
            if range.start > range.end {
                return Err(FactError::Reservation);
            }
        }
        Ok(())
    }
}
impl FactDelta {
    pub fn validate<'a>(
        &'a self,
        base: &'a CheckedFactSet<'a>,
        authority: &'a FactAuthority,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<CheckedFactDelta<'a>, FactError> {
        let previous = base.value;
        budget.charge(
            Resource::Work,
            (self.analysis_id.len() + authority.analysis_id.len() + previous.analysis_id.len())
                as u64
                + 1,
        )?;
        if self.analysis_id != previous.analysis_id
            || authority.analysis_id != previous.analysis_id
            || self.origin_base != previous.origins.len() as u64
        {
            return Err(FactError::Analysis);
        }
        let view = View::new(previous, Some(self), budget, admission)?;
        view.validate(base.registry, budget)?;
        let existing_scope = |id: ScopeId, budget: &mut Budget| -> Result<(), FactError> {
            budget.charge(Resource::Work, previous.scopes.len() as u64 + 1)?;
            if !previous.scopes.iter().any(|v| v.id == id) {
                return Err(FactError::Authority);
            }
            Ok(())
        };
        existing_scope(authority.current_scope, budget)?;
        view.scope(authority.current_scope, budget)?;
        for ns in &authority.namespaces {
            view.namespace(*ns)?;
        }
        for scope in &authority.writable_scopes {
            existing_scope(*scope, budget)?;
            if !view.descendant(*scope, authority.current_scope, budget)? {
                return Err(FactError::Authority);
            }
        }
        for scope in &authority.import_scopes {
            existing_scope(*scope, budget)?;
            view.scope(*scope, budget)?;
        }
        fn reserved(
            range: IdRange,
            ids: impl Iterator<Item = u64>,
            budget: &mut Budget,
        ) -> Result<(), FactError> {
            budget.charge(Resource::Work, 1)?;
            if range.start > range.end {
                return Err(FactError::Reservation);
            }
            for id in ids {
                budget.charge(Resource::Work, 1)?;
                if id < range.start || id >= range.end {
                    return Err(FactError::Reservation);
                }
            }
            Ok(())
        }
        reserved(
            authority.reservation.scopes,
            self.scopes.iter().map(|v| v.id.0),
            budget,
        )?;
        reserved(
            authority.reservation.entities,
            self.entities.iter().map(|v| v.id.0),
            budget,
        )?;
        reserved(
            authority.reservation.occurrences,
            self.occurrences.iter().map(|v| v.id.0),
            budget,
        )?;
        reserved(
            authority.reservation.relations,
            self.relations.iter().map(|v| v.id.0),
            budget,
        )?;
        let created = |scope: ScopeId, budget: &mut Budget| -> Result<bool, FactError> {
            for v in &self.scopes {
                budget.charge(Resource::Work, 1)?;
                if v.id == scope {
                    return Ok(true);
                }
            }
            Ok(false)
        };
        let writable = |scope: ScopeId, budget: &mut Budget| -> Result<bool, FactError> {
            Ok(scope == authority.current_scope
                || has(&authority.writable_scopes, &scope, budget)?
                || created(scope, budget)?)
        };
        for scope in &self.scopes {
            let parent = scope.parent.ok_or(FactError::Authority)?;
            if parent != authority.current_scope && !created(parent, budget)? {
                return Err(FactError::Authority);
            }
        }
        for entity in &self.entities {
            if !writable(entity.scope, budget)?
                || !has(&authority.namespaces, &entity.namespace, budget)?
            {
                return Err(FactError::Authority);
            }
        }
        for occurrence in &self.occurrences {
            if !writable(occurrence.scope, budget)?
                || !has(&authority.namespaces, &occurrence.namespace, budget)?
            {
                return Err(FactError::Authority);
            }
        }
        for edge in &self.edges {
            match edge {
                ScopeEdge::Import {
                    from,
                    to,
                    namespace,
                } => {
                    if !writable(*from, budget)?
                        || !has(&authority.namespaces, namespace, budget)?
                        || !(writable(*to, budget)? || has(&authority.import_scopes, to, budget)?)
                    {
                        return Err(FactError::Authority);
                    }
                }
                ScopeEdge::Export { from, to, entity } => {
                    if !writable(*from, budget)?
                        || !writable(*to, budget)?
                        || !has(
                            &authority.namespaces,
                            &view.entity(*entity, budget)?.namespace,
                            budget,
                        )?
                    {
                        return Err(FactError::Authority);
                    }
                }
            }
        }
        for target in &authority.relation_sources {
            view.target(target, budget)?;
        }
        for relation in &self.relations {
            let allowed = match &relation.source {
                FactTarget::Scope(scope) => writable(*scope, budget)?,
                FactTarget::Entity(id) => {
                    let e = view.entity(*id, budget)?;
                    writable(e.scope, budget)? && has(&authority.namespaces, &e.namespace, budget)?
                }
                FactTarget::Occurrence(id) => {
                    let e = view.occurrence(*id, budget)?;
                    writable(e.scope, budget)? && has(&authority.namespaces, &e.namespace, budget)?
                }
                FactTarget::Source(_) => false,
            };
            if !allowed {
                let mut granted = false;
                for target in &authority.relation_sources {
                    let size = |v: &FactTarget| match v {
                        FactTarget::Source(s) => s.snapshot_ref().source.0.len() as u64 + 41,
                        _ => 1,
                    };
                    budget.charge(Resource::Work, size(target).max(size(&relation.source)))?;
                    if target == &relation.source {
                        granted = true;
                        break;
                    }
                }
                if !granted {
                    return Err(FactError::Authority);
                }
            }
        }
        for (i, update) in self.resolutions.iter().enumerate() {
            budget.charge(Resource::Work, i as u64 + 1)?;
            budget.charge(Resource::Work, previous.occurrences.len() as u64 + 1)?;
            if !previous
                .occurrences
                .iter()
                .any(|v| v.id == update.occurrence)
            {
                return Err(FactError::Authority);
            }
            if self.resolutions[..i]
                .iter()
                .any(|v| v.occurrence == update.occurrence)
                || !has(&authority.resolution_updates, &update.occurrence, budget)?
            {
                return Err(FactError::Authority);
            }
            let occurrence = view.occurrence(update.occurrence, budget)?;
            if !writable(occurrence.scope, budget)?
                || !has(&authority.namespaces, &occurrence.namespace, budget)?
            {
                return Err(FactError::Authority);
            }
            view.resolution(occurrence, &update.resolution, base.registry, budget)?;
        }
        Ok(CheckedFactDelta {
            base: previous,
            delta: self,
            authority,
        })
    }
}
