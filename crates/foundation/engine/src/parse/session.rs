use super::{
    build, copy,
    environment::{self, ParseEnvironmentSet},
    error::ParseError,
    model::*,
    select,
};
use crate::{
    package::ReadSpec,
    profile::ResolvedParseProfile,
    recovery::{
        BundleRecovery, ForeignStep, ParseTree, RecoveryEntry, RecoveryKind, UnparsedReason,
    },
    selection::{BundleContext, NodeSelection, ShapeSelection},
};
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason, Usage},
    diagnostic::Report,
    origin::{Origin, OriginId},
    source::{SourceAdmission, SourceError, SourceReservation, SourceSnapshot, SourceStore},
    syntax::{EnvironmentRef, FieldValue, ForeignSyntax, NodeRef, SyntaxNode},
    value::KindRef,
    view::Token,
};
use nepl3_reader::{runtime::ProviderReply, tokenizer::*};

struct Machine {
    progress: ParseProgress,
    accepted: Option<AcceptedTokenizationReport>,
}
struct Incomplete {
    machine: Machine,
    usage: Usage,
    limits: Limits,
    depth_base: u64,
}
struct Pending {
    machine: Machine,
    tokenizer: Box<TokenizationContinuation>,
    usage: Usage,
    limits: Limits,
    depth_base: u64,
}
enum Halt {
    Done,
    NeedMore(Vec<nepl3_reader::model::Expectation>),
    Await(
        Box<nepl3_reader::model::ProviderCall>,
        Box<TokenizationContinuation>,
    ),
    Reserve(ReservationRequest, Box<TokenizationContinuation>),
}
pub struct ParseSession<'a> {
    session_id: String,
    profile: &'a ResolvedParseProfile<'a>,
    environments: &'a ParseEnvironmentSet<'a>,
    tokenizers: Vec<TokenizationSession<'a>>,
    pending: Option<Pending>,
    incomplete: Option<Incomplete>,
    next_operation: u64,
    closed: bool,
}
impl<'a> ParseSession<'a> {
    pub fn new(
        session_id: String,
        profile: &'a ResolvedParseProfile<'a>,
        environments: &'a ParseEnvironmentSet<'a>,
        budget: &mut Budget,
    ) -> Result<Self, ParseError> {
        if session_id.is_empty() {
            return Err(ParseError::Context);
        }
        let mut tokenizers = Vec::new();
        for registration in &profile.profile().languages {
            let checked = profile.checked(&registration.alias, budget)?;
            if !environments
                .languages
                .iter()
                .any(|v| v.alias == registration.alias)
            {
                return Err(ParseError::Context);
            }
            budget.charge(
                Resource::AllocationUnits,
                (session_id.len() + registration.alias.len() + 1) as u64,
            )?;
            build::slot::<TokenizationSession<'a>>(budget)?;
            tokenizers.push(TokenizationSession::new(
                alloc::format!("{session_id}:{}", registration.alias),
                &checked.package().modes,
                checked.reader(),
                profile.registry(),
                budget,
            )?);
        }
        Ok(Self {
            session_id,
            profile,
            environments,
            tokenizers,
            pending: None,
            incomplete: None,
            next_operation: 0,
            closed: false,
        })
    }
    pub fn close(&mut self) {
        self.closed = true;
        self.pending = None;
        self.incomplete = None;
        for tokenizer in &mut self.tokenizers {
            tokenizer.close();
        }
    }
    pub fn read(
        &mut self,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        if self.closed {
            return Err(ParseError::Closed);
        }
        if self.pending.is_some() || self.incomplete.is_some() {
            return Err(ParseError::Busy);
        }
        self.start(request, sources, budget, admission, None)
    }
    fn start(
        &mut self,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        operation: Option<&str>,
    ) -> Result<ParseReply, ParseError> {
        let prepared = self.begin(request, sources, budget, admission, operation);
        let mut machine = match prepared {
            Ok(v) => v,
            Err(error) => {
                return match budget.poll() {
                    Err(reason) => Ok(ParseReply {
                        outcome: ParseOutcome::Stopped {
                            reason,
                            progress: None,
                        },
                        report: Report {
                            usage: budget.usage(),
                            ..Report::default()
                        },
                        sources: vec![],
                        source_maps: vec![],
                    }),
                    Ok(()) => Err(error),
                };
            }
        };
        let Some(depth_base) = budget.current_depth().checked_add(1) else {
            let reason = budget.stop(StopReason::DepthLimit);
            return self.stop(machine, reason, budget);
        };
        let result =
            budget.with_depth(|budget| self.drive(&mut machine, sources, budget, admission));
        self.finish(machine, result, depth_base, budget)
    }
    fn begin(
        &mut self,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        operation: Option<&str>,
    ) -> Result<Machine, ParseError> {
        if !limits_within(budget.limits(), self.profile.profile().limits) {
            return Err(ParseError::LimitsMismatch);
        }
        budget.poll()?;
        request.snapshot.check_range(request.start, request.limit)?;
        budget.charge(
            Resource::Work,
            sources.snapshots().len() as u64
                * (request.snapshot.identity().source.0.len() as u64 + 33)
                + request.snapshot.uri().len() as u64,
        )?;
        if sources
            .get_ref(request.snapshot.identity())
            .is_none_or(|v| v.uri() != request.snapshot.uri())
        {
            return Err(SourceError::MissingSnapshot.into());
        }
        self.profile.validate_entry(request.entry, budget)?;
        admission.admit_existing(request.snapshot, budget)?;
        if request.states.len() != self.profile.profile().languages.len() {
            return Err(ParseError::State);
        }
        for registration in &self.profile.profile().languages {
            let mut matches = request
                .states
                .iter()
                .filter(|v| v.alias == registration.alias);
            let state = matches.next().ok_or(ParseError::State)?;
            if matches.next().is_some() {
                return Err(ParseError::State);
            }
            self.profile.registry().validate(
                &self
                    .profile
                    .language(&registration.alias, budget)?
                    .reader
                    .state_type,
                &state.state,
                budget,
            )?;
        }
        self.next_operation = self
            .next_operation
            .checked_add(1)
            .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
        budget.charge(
            Resource::AllocationUnits,
            self.session_id.len() as u64
                + 22
                + 2 * request.snapshot.identity().source.0.len() as u64,
        )?;
        let scope = TokenizationScope {
            operation_id: if let Some(id) = operation {
                build::text(id, budget)?
            } else {
                alloc::format!("{}:{}", self.session_id, self.next_operation)
            },
            profile_digest: self.profile.digest(),
            snapshot: request.snapshot.reference(),
        };
        budget.charge(
            Resource::AllocationUnits,
            (scope.operation_id.len() + scope.snapshot.source_id.0.len()) as u64,
        )?;
        let accepted = AcceptedTokenizationReport::empty(scope.clone(), budget)?;
        let states = copy::states(request.states, budget)?;
        let arena = self.arena(request.snapshot, Vec::new(), budget)?;
        build::slot::<ParseArena>(budget)?;
        build::slot::<ParseFrame>(budget)?;
        let mut environment_refs = Vec::new();
        for language in &self.environments.languages {
            build::slot::<LanguageEnvironmentRef>(budget)?;
            environment_refs.push(LanguageEnvironmentRef {
                alias: build::text(&language.alias, budget)?,
                environment: EnvironmentRef {
                    id: language.context.environment.id,
                    digest: language.context.environment.digest,
                },
            });
        }
        let mut environment_entries = Vec::new();
        for value in &arena.environments {
            environment_entries.push(value.clone_with_budget(budget)?);
        }
        let mut declared_sources = Vec::new();
        for source in &arena.sources {
            declared_sources.push(source.clone_with_budget(budget)?);
        }
        let declared_origins = environment::origins(&arena.origins, 0, budget)?;
        Ok(Machine {
            accepted: Some(accepted),
            progress: ParseProgress {
                request: OwnedParseRequest {
                    snapshot: request.snapshot.reference(),
                    start: request.start,
                    limit: request.limit,
                    final_input: request.final_input,
                    entry: copy::entry(request.entry, budget)?,
                    states: copy::states(request.states, budget)?,
                    environments: environment_refs,
                    environment_entries,
                    origins: declared_origins,
                    sources: declared_sources,
                },
                scope,
                cursor: request.start,
                states,
                arenas: vec![arena],
                frames: vec![ParseFrame {
                    entry: copy::entry(request.entry, budget)?,
                    read: None,
                    selection: None,
                    node: None,
                    arity: 0,
                    children: vec![],
                    next_child: 0,
                    arena: 0,
                    foreign: false,
                }],
                facts: vec![],
                recovery: vec![],
                contexts: vec![],
            },
        })
    }
    fn arena(
        &self,
        input: &SourceSnapshot,
        path: Vec<ForeignStep>,
        budget: &mut Budget,
    ) -> Result<ParseArena, ParseError> {
        let mut arena = ParseArena {
            path,
            ..ParseArena::default()
        };
        arena.source(input, budget)?;
        arena.origins = environment::origins(&self.environments.origins, 0, budget)?;
        for entry in &self.environments.entries {
            build::slot::<nepl3_core::syntax::EnvironmentEntry>(budget)?;
            arena.environments.push(entry.clone_with_budget(budget)?);
        }
        for language in &self.environments.languages {
            for source in language.context.sources() {
                arena.source(source, budget)?;
            }
        }
        Ok(arena)
    }
    fn alias(&self, alias: &str, budget: &mut Budget) -> Result<usize, ParseError> {
        budget.charge(
            Resource::Work,
            self.profile.profile().languages.len() as u64 * (alias.len() as u64 + 1),
        )?;
        self.profile
            .profile()
            .languages
            .iter()
            .position(|v| v.alias == alias)
            .ok_or(ParseError::Context)
    }
    fn drive(
        &mut self,
        machine: &mut Machine,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Halt, ParseError> {
        loop {
            budget.poll()?;
            budget.observe_depth(machine.progress.frames.len() as u64)?;
            let Some(frame) = machine.progress.frames.last() else {
                self.validate_completed(machine, budget, admission)?;
                return Ok(Halt::Done);
            };
            if frame.node.is_some() {
                if frame.children.len() as u64 == frame.arity {
                    self.complete(machine, budget)?;
                    continue;
                }
                self.child(machine, sources, budget)?;
                continue;
            }
            let entry = copy::entry(&frame.entry, budget)?;
            let alias = self.alias(&entry.alias, budget)?;
            let package = self.profile.language(&entry.alias, budget)?;
            let target = match frame.read.map(|id| package.read(id)).transpose()? {
                Some(ReadSpec::Builtin {
                    reader, token_kind, ..
                }) => {
                    budget.charge(
                        Resource::AllocationUnits,
                        token_kind.schema.package.len() as u64,
                    )?;
                    TokenTarget::Builtin {
                        reader: *reader,
                        token_kind: token_kind.clone(),
                    }
                }
                _ => TokenTarget::Mode,
            };
            let original = self
                .environments
                .languages
                .iter()
                .find(|v| v.alias == entry.alias)
                .ok_or(ParseError::Context)?;
            let context = original
                .context
                .retarget(
                    &entry.package.schema,
                    &entry.category,
                    &entry.mode,
                    sources,
                    self.profile.registry(),
                    budget,
                )
                .map_err(|_| ParseError::Context)?;
            let snapshot = sources
                .resolve(&machine.progress.request.snapshot)
                .ok_or(SourceError::MissingSnapshot)?;
            let state = machine
                .progress
                .states
                .iter()
                .find(|v| v.alias == entry.alias)
                .ok_or(ParseError::State)?;
            build::slot::<ReaderFactBatch>(budget)?;
            machine.progress.facts.reserve(1);
            let path = copy_path(
                &machine
                    .progress
                    .arenas
                    .last()
                    .ok_or(ParseError::Reference)?
                    .path,
                budget,
            )?;
            let batch = ReaderFactBatch {
                path,
                node: None,
                entry,
                facts: vec![],
                trivia: vec![],
            };
            let backup = machine
                .accepted
                .as_ref()
                .ok_or(ParseError::Reference)?
                .checkpoint(budget)?;
            let accepted = machine.accepted.take().ok_or(ParseError::Reference)?;
            machine.accepted = Some(backup);
            let active_depth = budget
                .current_depth()
                .checked_add(machine.progress.frames.len() as u64)
                .ok_or_else(|| budget.stop(StopReason::DepthLimit))?;
            let result = budget.with_depth_at_least(active_depth, |budget| {
                self.tokenizers[alias].read_with_accepted(
                    ScopedTokenizationRequest {
                        scope: &machine.progress.scope,
                        target,
                        input: TokenizationRequest {
                            snapshot,
                            start: machine.progress.cursor,
                            limit: machine.progress.request.limit,
                            final_input: machine.progress.request.final_input,
                            context: &context,
                            state: &state.state,
                        },
                    },
                    sources,
                    budget,
                    admission,
                    accepted,
                )
            })?;
            machine.progress.facts.push(batch);
            if let Some(halt) = self.accept(machine, result, sources, budget, admission)? {
                return Ok(halt);
            }
        }
    }
    fn accept(
        &mut self,
        machine: &mut Machine,
        reply: AcceptedTokenizationReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<Halt>, ParseError> {
        let AcceptedTokenizationReply {
            outcome,
            cursor,
            new_state,
            trivia,
            facts,
            accepted,
        } = reply;
        machine.accepted = Some(accepted);
        machine.progress.cursor = cursor;
        let frame = machine
            .progress
            .frames
            .last()
            .ok_or(ParseError::Reference)?;
        let state = machine
            .progress
            .states
            .iter_mut()
            .find(|v| v.alias == frame.entry.alias)
            .ok_or(ParseError::State)?;
        if let Some(value) = new_state {
            state.state = value;
        }
        machine
            .progress
            .facts
            .last_mut()
            .ok_or(ParseError::Reference)?
            .facts = facts;
        machine
            .progress
            .facts
            .last_mut()
            .ok_or(ParseError::Reference)?
            .trivia = trivia;
        match outcome {
            TokenizationOutcome::Token(token) => {
                self.token(machine, token, sources, budget, admission)?;
                Ok(None)
            }
            TokenizationOutcome::Await { call, continuation } => {
                Ok(Some(Halt::Await(call, continuation)))
            }
            TokenizationOutcome::Reserve {
                request,
                continuation,
            } => Ok(Some(Halt::Reserve(request, continuation))),
            TokenizationOutcome::NeedMore { expected } => Ok(Some(Halt::NeedMore(expected))),
            TokenizationOutcome::Stopped { reason } => Err(reason.into()),
            TokenizationOutcome::End => {
                self.recover(machine, None, true, sources, budget, admission)?;
                Ok(None)
            }
            TokenizationOutcome::NoMatch { .. } | TokenizationOutcome::Failed { .. } => {
                self.recover(machine, None, false, sources, budget, admission)?;
                Ok(None)
            }
        }
    }
    fn token(
        &self,
        machine: &mut Machine,
        token: Token,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), ParseError> {
        let frame = machine
            .progress
            .frames
            .last()
            .ok_or(ParseError::Reference)?;
        let package = self.profile.language(&frame.entry.alias, budget)?;
        let snapshot = sources
            .resolve(&machine.progress.request.snapshot)
            .ok_or(SourceError::MissingSnapshot)?;
        let chosen = match frame.read {
            None => select::category(
                package,
                &frame.entry,
                &token,
                snapshot,
                self.profile.registry(),
                budget,
            )?,
            Some(id) => match package.read(id)? {
                ReadSpec::Builtin { kind, .. } => Some(select::Head {
                    kind,
                    selection: ShapeSelection::Builtin { read: id },
                    arity: 0,
                }),
                ReadSpec::ListOf { cons, nil, .. } => match snapshot.slice(&token.head)? {
                    "cons" => Some(select::Head {
                        kind: cons,
                        selection: ShapeSelection::List {
                            read: id,
                            cons: true,
                        },
                        arity: 2,
                    }),
                    "nil" => Some(select::Head {
                        kind: nil,
                        selection: ShapeSelection::List {
                            read: id,
                            cons: false,
                        },
                        arity: 0,
                    }),
                    _ => None,
                },
                _ => return Err(ParseError::Reference),
            },
        };
        let Some(chosen) = chosen else {
            return self.recover(machine, Some(token), false, sources, budget, admission);
        };
        let selection = chosen.selection;
        let entry = copy::entry(&frame.entry, budget)?;
        let execution_digest = self.profile.execution_digest(&entry.alias, budget)?;
        let arena = machine
            .progress
            .arenas
            .last_mut()
            .ok_or(ParseError::Reference)?;
        let accepted = machine.accepted.as_ref().ok_or(ParseError::Reference)?;
        for source in accepted.sources() {
            arena.source(source, budget)?;
        }
        for mapping in accepted.source_maps() {
            if !arena.source_maps.contains(mapping) {
                build::slot::<nepl3_core::origin::Mapping>(budget)?;
                arena.source_maps.push(mapping.clone_with_budget(budget)?);
            }
        }
        build::slot::<NodeSelection>(budget)?;
        let node = arena.head(token, chosen.kind, self.profile.registry(), budget)?;
        arena.selections.push(NodeSelection {
            node,
            entry,
            execution_digest,
            shape: selection.clone(),
        });
        let frame = machine
            .progress
            .frames
            .last_mut()
            .ok_or(ParseError::Reference)?;
        frame.node = Some(node);
        frame.arity = chosen.arity;
        frame.selection = Some(selection);
        machine
            .progress
            .facts
            .last_mut()
            .ok_or(ParseError::Reference)?
            .node = Some(node);
        Ok(())
    }
    fn child(
        &self,
        machine: &mut Machine,
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<(), ParseError> {
        let parent = machine
            .progress
            .frames
            .last()
            .ok_or(ParseError::Reference)?;
        let package = self.profile.language(&parent.entry.alias, budget)?;
        let index = parent.children.len();
        let (read, field, tail) = match parent.selection.as_ref().ok_or(ParseError::Reference)? {
            ShapeSelection::Form { index: form } => {
                let field = package
                    .forms
                    .get(usize::try_from(*form).map_err(|_| ParseError::Reference)?)
                    .and_then(|v| v.fields.get(index))
                    .ok_or(ParseError::Reference)?;
                (field.read, field.name.as_str(), false)
            }
            ShapeSelection::List { read, cons: true } => {
                let ReadSpec::ListOf { element, .. } = package.read(*read)? else {
                    return Err(ParseError::Reference);
                };
                if index == 0 {
                    (*element, "head", false)
                } else {
                    (*read, "tail", true)
                }
            }
            _ => return Err(ParseError::Reference),
        };
        let resolved = if tail {
            crate::profile::ResolvedRead {
                entry: copy::entry(&parent.entry, budget)?,
                read: Some(read),
                foreign: false,
            }
        } else {
            select::read(self.profile, &parent.entry, read, budget)?
        };
        let arena = if resolved.foreign {
            let mut path = copy_path(
                &machine
                    .progress
                    .arenas
                    .last()
                    .ok_or(ParseError::Reference)?
                    .path,
                budget,
            )?;
            build::slot::<ForeignStep>(budget)?;
            path.push(ForeignStep {
                node: parent.node.ok_or(ParseError::Reference)?,
                field: build::text(field, budget)?,
            });
            let source = sources
                .resolve(&machine.progress.request.snapshot)
                .ok_or(SourceError::MissingSnapshot)?;
            let arena = self.arena(source, path, budget)?;
            build::slot::<ParseArena>(budget)?;
            machine.progress.arenas.push(arena);
            machine.progress.arenas.len() as u64 - 1
        } else {
            parent.arena
        };
        build::slot::<ParseFrame>(budget)?;
        machine.progress.frames.push(ParseFrame {
            entry: resolved.entry,
            read: resolved.read,
            selection: None,
            node: None,
            arity: 0,
            children: vec![],
            next_child: 0,
            arena,
            foreign: resolved.foreign,
        });
        Ok(())
    }
    fn complete(&self, machine: &mut Machine, budget: &mut Budget) -> Result<(), ParseError> {
        let frame = machine
            .progress
            .frames
            .last()
            .ok_or(ParseError::Reference)?;
        let node = frame.node.ok_or(ParseError::Reference)?;
        let arena = machine
            .progress
            .arenas
            .last_mut()
            .ok_or(ParseError::Reference)?;
        // Prepare all allocations before releasing the completed frame.
        if machine.progress.frames.len() > 1 {
            build::slot::<FieldValue>(budget)?;
        }
        if frame.foreign {
            build::slot::<ForeignSyntax>(budget)?;
            build::slot::<BundleContext>(budget)?;
        }
        let fields = &mut machine
            .progress
            .frames
            .last_mut()
            .ok_or(ParseError::Reference)?
            .children;
        arena.complete(node, fields, machine.progress.cursor, budget)?;
        let frame = machine.progress.frames.pop().ok_or(ParseError::Reference)?;
        if machine.progress.frames.is_empty() {
            arena.root = Some(node);
            return Ok(());
        }
        let value = if frame.foreign {
            let mut arena = machine.progress.arenas.pop().ok_or(ParseError::Reference)?;
            let context = BundleContext {
                path: core::mem::take(&mut arena.path),
                nodes: core::mem::take(&mut arena.selections),
            };
            machine.progress.contexts.push(context);
            let environment = &self
                .environments
                .languages
                .iter()
                .find(|v| v.alias == frame.entry.alias)
                .ok_or(ParseError::Context)?
                .context
                .environment;
            FieldValue::Foreign(Box::new(ForeignSyntax {
                schema: frame.entry.package.schema,
                category: frame.entry.category,
                root: node,
                bundle: arena.finish(node),
                environment: EnvironmentRef {
                    id: environment.id,
                    digest: environment.digest,
                },
            }))
        } else {
            FieldValue::Child(node)
        };
        let parent = machine
            .progress
            .frames
            .last_mut()
            .ok_or(ParseError::Reference)?;
        parent.children.push(value);
        parent.next_child = parent.children.len() as u64;
        Ok(())
    }
    fn recover(
        &self,
        machine: &mut Machine,
        token: Option<Token>,
        missing: bool,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), ParseError> {
        let frame = machine
            .progress
            .frames
            .last()
            .ok_or(ParseError::Reference)?;
        let entry = copy::entry(&frame.entry, budget)?;
        let source = sources
            .resolve(&machine.progress.request.snapshot)
            .ok_or(SourceError::MissingSnapshot)?;
        let start = token
            .as_ref()
            .map(|v| v.head.start())
            .unwrap_or(machine.progress.cursor);
        let end = if missing {
            start
        } else {
            machine.progress.request.limit
        };
        let schema = self
            .profile
            .registry()
            .selected("nepl3.engine", 1)
            .ok_or(nepl3_core::schema::SchemaError::UnknownSchema)?;
        let name = if missing {
            "RecoveryMissing"
        } else {
            "RecoveryUnparsed"
        };
        let kind_id = self.profile.registry().kind_id(schema, name)?;
        budget.charge(Resource::AllocationUnits, schema.package.len() as u64)?;
        let kind = KindRef {
            schema: schema.clone(),
            local_kind: kind_id,
        };
        let cover = source.span_with_budget(start, end, budget)?;
        let diagnostic =
            super::diagnostic::recovery(&entry, &cover, missing, self.profile.registry(), budget)?;
        machine
            .accepted
            .as_mut()
            .ok_or(ParseError::Reference)?
            .diagnostic(
                diagnostic,
                sources,
                self.profile.registry(),
                budget,
                admission,
            )?;
        let detail = if missing {
            RecoveryKind::Missing {
                expected: copy::entry(&entry, budget)?,
                anchor: build::span(&cover, budget)?,
            }
        } else {
            RecoveryKind::Unparsed {
                span: build::span(&cover, budget)?,
                reason: UnparsedReason::UnknownHead,
            }
        };
        let arena = machine
            .progress
            .arenas
            .last_mut()
            .ok_or(ParseError::Reference)?;
        build::slot::<RecoveryEntry>(budget)?;
        build::slot::<BundleRecovery>(budget)?;
        build::slot::<NodeSelection>(budget)?;
        let path = copy_path(&arena.path, budget)?;
        let node = if let Some(token) = token {
            arena.head(token, &kind, self.profile.registry(), budget)?
        } else {
            budget.charge(Resource::Nodes, 1)?;
            build::slot::<SyntaxNode>(budget)?;
            build::slot::<Origin>(budget)?;
            let origin = OriginId(arena.origins.len() as u64);
            let id = NodeRef(arena.nodes.len() as u64);
            let kind = build::text(name, budget)?;
            let origin_span = build::span(&cover, budget)?;
            let node_cover = build::span(&cover, budget)?;
            budget.charge(Resource::AllocationUnits, schema.package.len() as u64)?;
            arena.origins.push(Origin::Direct(origin_span));
            arena.nodes.push(SyntaxNode {
                schema: schema.clone(),
                kind,
                fields: vec![],
                head: None,
                cover: Some(node_cover),
                origin,
                token: None,
            });
            id
        };
        // Recovery is a separate table, never a guessed leaf arity for an unknown head.
        let execution_digest = self.profile.execution_digest(&entry.alias, budget)?;
        arena.selections.push(NodeSelection {
            node,
            entry,
            execution_digest,
            shape: ShapeSelection::Recovery,
        });
        if let Some(bundle) = machine
            .progress
            .recovery
            .iter_mut()
            .find(|v| v.path == path)
        {
            bundle.entries.push(RecoveryEntry { node, kind: detail });
        } else {
            machine.progress.recovery.push(BundleRecovery {
                path,
                entries: vec![RecoveryEntry { node, kind: detail }],
            });
        }
        let frame = machine
            .progress
            .frames
            .last_mut()
            .ok_or(ParseError::Reference)?;
        frame.node = Some(node);
        frame.arity = 0;
        frame.selection = Some(ShapeSelection::Recovery);
        machine.progress.cursor = end;
        Ok(())
    }
    fn validate_completed(
        &self,
        machine: &Machine,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), ParseError> {
        // Validate a budgeted snapshot before moving the formal arenas out of the machine.
        // A stopped validation therefore leaves the original progress and report intact.
        let mut progress = machine.progress.clone_with_budget(budget)?;
        let mut arena = progress.arenas.pop().ok_or(ParseError::Reference)?;
        let root = arena.root.ok_or(ParseError::Reference)?;
        build::slot::<BundleContext>(budget)?;
        progress.contexts.push(BundleContext {
            path: core::mem::take(&mut arena.path),
            nodes: core::mem::take(&mut arena.selections),
        });
        let tree = ParseTree {
            profile_digest: self.profile.digest(),
            bundle: arena.finish(root),
            recovery: progress.recovery,
            contexts: progress.contexts,
        };
        tree.validate(self.profile, budget, admission)?;
        Ok(())
    }
    fn discard_pending(&mut self) {
        for tokenizer in &mut self.tokenizers {
            tokenizer.discard_pending();
        }
    }
    fn stop(
        &mut self,
        mut machine: Machine,
        reason: StopReason,
        budget: &Budget,
    ) -> Result<ParseReply, ParseError> {
        self.discard_pending();
        let accepted = machine.accepted.take().ok_or(ParseError::Reference)?;
        let (mut report, sources, source_maps) = accepted.into_parts();
        report.usage = budget.usage();
        Ok(ParseReply {
            outcome: ParseOutcome::Stopped {
                reason,
                progress: Some(machine.progress),
            },
            report,
            sources,
            source_maps,
        })
    }
    fn finish(
        &mut self,
        mut machine: Machine,
        result: Result<Halt, ParseError>,
        depth_base: u64,
        budget: &mut Budget,
    ) -> Result<ParseReply, ParseError> {
        let halt = match result {
            Ok(v) => v,
            Err(error) => {
                return match budget.poll() {
                    Err(reason) => self.stop(machine, reason, budget),
                    Ok(()) => match error {
                        ParseError::Stopped(reason) => self.stop(machine, reason, budget),
                        error => {
                            self.discard_pending();
                            Err(error)
                        }
                    },
                };
            }
        };
        match halt {
            Halt::Await(call, tokenizer) => {
                self.suspend(machine, Some(call), None, tokenizer, depth_base, budget)
            }
            Halt::Reserve(request, tokenizer) => {
                self.suspend(machine, None, Some(request), tokenizer, depth_base, budget)
            }
            Halt::NeedMore(expected) => {
                let prepared = (|| -> Result<_, ParseError> {
                    let progress = machine.progress.clone_with_budget(budget)?;
                    let accepted = machine
                        .accepted
                        .as_ref()
                        .ok_or(ParseError::Reference)?
                        .checkpoint(budget)?;
                    Ok((progress, accepted))
                })();
                let (progress, accepted) = match prepared {
                    Ok(v) => v,
                    Err(e) => return self.finish(machine, Err(e), depth_base, budget),
                };
                let usage = budget.usage();
                self.incomplete = Some(Incomplete {
                    machine,
                    usage,
                    limits: budget.limits(),
                    depth_base,
                });
                let (mut report, sources, source_maps) = accepted.into_parts();
                report.usage = usage;
                Ok(ParseReply {
                    outcome: ParseOutcome::NeedMore { expected, progress },
                    report,
                    sources,
                    source_maps,
                })
            }
            Halt::Done => {
                let prepared = (|| -> Result<(), ParseError> {
                    let arena = machine
                        .progress
                        .arenas
                        .last()
                        .ok_or(ParseError::Reference)?;
                    arena.root.ok_or(ParseError::Reference)?;
                    build::slot::<BundleContext>(budget)?;
                    Ok(())
                })();
                if let Err(error) = prepared {
                    return self.finish(machine, Err(error), depth_base, budget);
                }
                let mut arena = machine.progress.arenas.pop().ok_or(ParseError::Reference)?;
                let root = arena.root.ok_or(ParseError::Reference)?;
                let context = BundleContext {
                    path: core::mem::take(&mut arena.path),
                    nodes: core::mem::take(&mut arena.selections),
                };
                let bundle = arena.finish(root);
                machine.progress.contexts.push(context);
                let tree = ParseTree {
                    profile_digest: self.profile.digest(),
                    bundle,
                    recovery: machine.progress.recovery,
                    contexts: machine.progress.contexts,
                };
                let outcome = if tree.recovery.is_empty() {
                    ParseOutcome::Complete {
                        tree,
                        cursor: machine.progress.cursor,
                        states: machine.progress.states,
                        facts: machine.progress.facts,
                    }
                } else {
                    ParseOutcome::Recovered {
                        tree,
                        cursor: machine.progress.cursor,
                        states: machine.progress.states,
                        facts: machine.progress.facts,
                    }
                };
                let (mut report, sources, source_maps) = machine
                    .accepted
                    .take()
                    .ok_or(ParseError::Reference)?
                    .into_parts();
                report.usage = budget.usage();
                Ok(ParseReply {
                    outcome,
                    report,
                    sources,
                    source_maps,
                })
            }
        }
    }
    fn suspend(
        &mut self,
        machine: Machine,
        call: Option<Box<nepl3_reader::model::ProviderCall>>,
        reservation: Option<ReservationRequest>,
        tokenizer: Box<TokenizationContinuation>,
        depth_base: u64,
        budget: &mut Budget,
    ) -> Result<ParseReply, ParseError> {
        let prepared = (|| -> Result<_, ParseError> {
            let progress = machine.progress.clone_with_budget(budget)?;
            build::slot::<TokenizationContinuation>(budget)?;
            let inner = Box::new(tokenizer.clone_with_budget(budget)?);
            build::slot::<ParseContinuation>(budget)?;
            let session_id = build::text(&self.session_id, budget)?;
            let accepted = machine
                .accepted
                .as_ref()
                .ok_or(ParseError::Reference)?
                .checkpoint(budget)?;
            Ok((progress, inner, session_id, accepted))
        })();
        let (progress, inner, session_id, accepted) = match prepared {
            Ok(v) => v,
            Err(error) => return self.finish(machine, Err(error), depth_base, budget),
        };
        let usage = budget.usage();
        let continuation = Box::new(ParseContinuation {
            session_id,
            progress,
            tokenizer: inner,
            usage,
            depth_base,
        });
        let outcome = match (call, reservation) {
            (Some(call), None) => ParseOutcome::Await { call, continuation },
            (None, Some(request)) => ParseOutcome::Reserve {
                request,
                continuation,
            },
            _ => return Err(ParseError::Reference),
        };
        self.pending = Some(Pending {
            machine,
            tokenizer,
            usage,
            limits: budget.limits(),
            depth_base,
        });
        let (mut report, sources, source_maps) = accepted.into_parts();
        report.usage = usage;
        Ok(ParseReply {
            outcome,
            report,
            sources,
            source_maps,
        })
    }
    /// Retry an append-only nonfinal input from its original reader states.
    /// Previously accepted candidates are recomputed; logical usage and source admission
    /// remain cumulative. This is a full reparse, not an incremental performance claim.
    pub fn continue_input(
        &mut self,
        echo: &ParseProgress,
        request: ParseRequest<'_>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        if self.closed {
            return Err(ParseError::Closed);
        }
        let pending = self.incomplete.as_ref().ok_or(ParseError::NoPending)?;
        if pending.limits != budget.limits()
            || !usage_at_least(budget.usage(), pending.usage)
            || echo.scope.operation_id != pending.machine.progress.scope.operation_id
        {
            return Err(ParseError::Continuation);
        }
        if let Err(reason) = budget.poll() {
            let pending = self.incomplete.take().ok_or(ParseError::NoPending)?;
            return self.stop(pending.machine, reason, budget);
        }
        let checked = (|| -> Result<(), ParseError> {
            echo.charge_clone(budget)?;
            let old = &pending.machine.progress.request;
            if echo != &pending.machine.progress
                || old.final_input
                || request.start != old.start
                || request.limit < old.limit
                || request.entry != &old.entry
                || request.states != old.states
            {
                return Err(ParseError::Continuation);
            }
            request.snapshot.check_range(request.start, request.limit)?;
            let old_source = old
                .sources
                .iter()
                .find(|v| {
                    v.identity().source == old.snapshot.source_id
                        && v.identity().revision == old.snapshot.revision
                        && v.identity().digest == old.snapshot.digest
                })
                .ok_or(ParseError::Reference)?;
            budget.charge(
                Resource::Work,
                old.limit
                    + old_source.uri().len() as u64
                    + old.snapshot.source_id.0.len() as u64
                    + 33,
            )?;
            if request.snapshot.identity().source != old.snapshot.source_id
                || request.snapshot.identity().revision < old.snapshot.revision
                || request.snapshot.uri() != old_source.uri()
                || request
                    .snapshot
                    .text()
                    .as_bytes()
                    .get(..usize::try_from(old.limit).map_err(|_| ParseError::Continuation)?)
                    != Some(old_source.slice_range(0, old.limit)?.as_bytes())
            {
                return Err(ParseError::Continuation);
            }
            if request.snapshot.identity().revision == old.snapshot.revision
                && request.snapshot.identity().digest != old.snapshot.digest
            {
                return Err(ParseError::Continuation);
            }
            Ok(())
        })();
        if let Err(error) = checked {
            return match budget.poll() {
                Err(reason) => {
                    let pending = self.incomplete.take().ok_or(ParseError::NoPending)?;
                    self.stop(pending.machine, reason, budget)
                }
                Ok(()) => Err(error),
            };
        }
        let pending = self.incomplete.take().ok_or(ParseError::NoPending)?;
        self.discard_pending();
        budget.with_depth_at_least(pending.depth_base.saturating_sub(1), |budget| {
            self.start(
                request,
                sources,
                budget,
                admission,
                Some(&pending.machine.progress.scope.operation_id),
            )
        })
    }
    pub fn reserve(
        &mut self,
        echo: &ParseContinuation,
        reservation: &SourceReservation,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        self.resume_with(echo, None, Some(reservation), sources, budget, admission)
    }
    pub fn resume(
        &mut self,
        echo: &ParseContinuation,
        reply: ProviderReply,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        self.resume_with(echo, Some(reply), None, sources, budget, admission)
    }
    fn resume_with(
        &mut self,
        echo: &ParseContinuation,
        provider: Option<ProviderReply>,
        reservation: Option<&SourceReservation>,
        sources: &SourceStore,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ParseReply, ParseError> {
        if self.closed {
            return Err(ParseError::Closed);
        }
        let pending = self.pending.as_ref().ok_or(ParseError::NoPending)?;
        if pending.limits != budget.limits()
            || !usage_at_least(budget.usage(), pending.usage)
            || echo.session_id != self.session_id
        {
            return Err(ParseError::Continuation);
        }
        if let Err(reason) = budget.poll() {
            let pending = self.pending.take().ok_or(ParseError::NoPending)?;
            return self.stop(pending.machine, reason, budget);
        }
        let checked = (|| -> Result<(), ParseError> {
            echo.progress.charge_clone(budget)?;
            echo.tokenizer.charge_clone(budget)?;
            Ok(())
        })();
        if let Err(error) = checked {
            let pending = self.pending.take().ok_or(ParseError::NoPending)?;
            return self.finish(pending.machine, Err(error), pending.depth_base, budget);
        }
        let pending = self.pending.as_ref().ok_or(ParseError::NoPending)?;
        if echo.progress != pending.machine.progress
            || echo.tokenizer != pending.tokenizer
            || echo.usage != pending.usage
            || echo.depth_base != pending.depth_base
        {
            return Err(ParseError::Continuation);
        }
        let pending = self.pending.as_ref().ok_or(ParseError::NoPending)?;
        if !matches!(
            (
                &pending.tokenizer.pending,
                provider.is_some(),
                reservation.is_some()
            ),
            (TokenizationWait::Provider { .. }, true, false)
                | (TokenizationWait::Reservation { .. }, false, true)
        ) {
            return Err(ParseError::Continuation);
        }
        let alias_result = self.alias(
            &pending
                .machine
                .progress
                .frames
                .last()
                .ok_or(ParseError::Reference)?
                .entry
                .alias,
            budget,
        );
        let mut pending = self.pending.take().ok_or(ParseError::NoPending)?;
        let alias = match alias_result {
            Ok(v) => v,
            Err(e) => return self.finish(pending.machine, Err(e), pending.depth_base, budget),
        };
        let result = match (provider, reservation) {
            (Some(reply), None) => self.tokenizers[alias].resume_accepted(
                &pending.tokenizer,
                reply,
                sources,
                budget,
                admission,
            ),
            (None, Some(reservation)) => self.tokenizers[alias].reserve_accepted(
                &pending.tokenizer,
                reservation,
                sources,
                budget,
                admission,
            ),
            _ => return Err(ParseError::Continuation),
        };
        let result = match result {
            Ok(reply) => match self.accept(&mut pending.machine, reply, sources, budget, admission)
            {
                Ok(Some(halt)) => Ok(halt),
                Ok(None) => budget.with_depth_at_least(pending.depth_base, |budget| {
                    self.drive(&mut pending.machine, sources, budget, admission)
                }),
                Err(error) => Err(error),
            },
            Err(error) => Err(error.into()),
        };
        self.finish(pending.machine, result, pending.depth_base, budget)
    }
}
fn copy_path(path: &[ForeignStep], budget: &mut Budget) -> Result<Vec<ForeignStep>, StopReason> {
    let mut out = Vec::new();
    for step in path {
        build::slot::<ForeignStep>(budget)?;
        out.push(ForeignStep {
            node: step.node,
            field: build::text(&step.field, budget)?,
        });
    }
    Ok(out)
}

fn usage_at_least(a: Usage, b: Usage) -> bool {
    a.source_bytes >= b.source_bytes
        && a.work >= b.work
        && a.depth >= b.depth
        && a.nodes >= b.nodes
        && a.allocation_units >= b.allocation_units
        && a.output_bytes >= b.output_bytes
        && a.diagnostics >= b.diagnostics
        && a.events >= b.events
}

fn limits_within(a: Limits, b: Limits) -> bool {
    a.source_bytes <= b.source_bytes
        && a.work <= b.work
        && a.depth <= b.depth
        && a.nodes <= b.nodes
        && a.allocation_units <= b.allocation_units
        && a.output_bytes <= b.output_bytes
        && a.diagnostics <= b.diagnostics
        && a.events <= b.events
}
