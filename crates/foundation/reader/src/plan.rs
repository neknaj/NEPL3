//! Arena-owned reader definitions. Static validation precedes execution.
mod identity;
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor},
    value::{KindRef, OperationRef, SchemaRef},
    view::PresentationClass,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReaderId(pub u64);
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharClass {
    Any,
    Whitespace,
    IdentifierStart,
    IdentifierContinue,
    Digit,
    AsciiLetter,
    Chars(String),
    Except(String),
    Range { lo: char, hi: char },
}
impl CharClass {
    /// Unicode identifier tables are pinned to Unicode16.0.0; no normalization is applied.
    pub fn contains(&self, c: char) -> bool {
        match self {
            Self::Any => true,
            Self::Whitespace => matches!(c, ' ' | '\t' | '\r' | '\n'),
            Self::IdentifierStart => c == '_' || unicode_ident::is_xid_start(c),
            Self::IdentifierContinue => unicode_ident::is_xid_continue(c),
            Self::Digit => c.is_ascii_digit(),
            Self::AsciiLetter => c.is_ascii_alphabetic(),
            Self::Chars(chars) => chars.contains(c),
            Self::Except(chars) => !chars.contains(c),
            Self::Range { lo, hi } => *lo <= c && c <= *hi,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    Read,
    Transform,
    Dependent,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSignature {
    pub operation: OperationRef,
    pub kind: ProviderKind,
    pub value_input: TypeDescriptor,
    pub value_output: TypeDescriptor,
    pub pure: bool,
    pub state_type: TypeDescriptor,
    pub continuation_type: TypeDescriptor,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReaderExpr {
    Literal(String),
    Scalar(CharClass),
    Seq(Vec<ReaderId>),
    Choice(Vec<ReaderId>),
    Many(ReaderId),
    Some(ReaderId),
    Optional(ReaderId),
    Repeat {
        min: u64,
        max: u64,
        body: ReaderId,
    },
    Look(ReaderId),
    Not(ReaderId),
    Commit(ReaderId),
    Capture {
        name: String,
        body: ReaderId,
    },
    Region {
        class: PresentationClass,
        body: ReaderId,
    },
    Node {
        kind: KindRef,
        body: ReaderId,
    },
    Discard(ReaderId),
    Ref(String),
    Decode {
        provider: OperationRef,
        body: ReaderId,
    },
    Map {
        provider: OperationRef,
        body: ReaderId,
    },
    Then {
        first: ReaderId,
        provider: OperationRef,
    },
    Call(OperationRef),
    Eof,
    TakeCount(u64),
    Until(String),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderRule {
    pub name: String,
    pub root: ReaderId,
    pub output: TypeDescriptor,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderPlan {
    pub schema: SchemaRef,
    pub state_type: TypeDescriptor,
    pub expressions: Vec<ReaderExpr>,
    pub rules: Vec<ReaderRule>,
    pub providers: Vec<ProviderSignature>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanError {
    Schema(SchemaError),
    Stopped(StopReason),
    Reference,
    DuplicateRule,
    DuplicateProvider,
    EmptyName,
    InvalidRange,
    InvalidRepeat,
    EmptyDelimiter,
    UnknownSchema,
    UnknownKind,
    ProviderSignature,
    OutputType,
    NonProgress,
    LeftRecursion,
}
impl From<SchemaError> for PlanError {
    fn from(e: SchemaError) -> Self {
        match e {
            SchemaError::Stopped(s) => Self::Stopped(s),
            e => Self::Schema(e),
        }
    }
}
impl From<StopReason> for PlanError {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
#[derive(Debug)]
pub struct CheckedPlan<'a> {
    registry: &'a SchemaRegistry,
    plan: &'a ReaderPlan,
    nullable: Vec<bool>,
    outputs: Vec<TypeDescriptor>,
}
impl<'a> CheckedPlan<'a> {
    pub fn registry(&self) -> &'a SchemaRegistry {
        self.registry
    }
    pub fn plan(&self) -> &'a ReaderPlan {
        self.plan
    }
    pub fn has_known_empty_success(&self, id: ReaderId) -> Option<bool> {
        usize::try_from(id.0)
            .ok()
            .and_then(|i| self.nullable.get(i))
            .copied()
    }
    pub fn output(&self, id: ReaderId) -> Option<&TypeDescriptor> {
        usize::try_from(id.0).ok().and_then(|i| self.outputs.get(i))
    }
}
impl ReaderPlan {
    pub fn expression(&self, id: ReaderId) -> Result<&ReaderExpr, PlanError> {
        usize::try_from(id.0)
            .ok()
            .and_then(|i| self.expressions.get(i))
            .ok_or(PlanError::Reference)
    }
    pub fn rule(&self, name: &str) -> Result<&ReaderRule, PlanError> {
        self.rules
            .iter()
            .find(|rule| rule.name == name)
            .ok_or(PlanError::Reference)
    }
    pub fn provider(
        &self,
        operation: &OperationRef,
        kind: ProviderKind,
    ) -> Result<&ProviderSignature, PlanError> {
        self.providers
            .iter()
            .find(|p| &p.operation == operation && p.kind == kind)
            .ok_or(PlanError::ProviderSignature)
    }
    pub fn check<'a>(
        &'a self,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<CheckedPlan<'a>, PlanError> {
        if !registry.is_finalized() || registry.descriptor(&self.schema).is_none() {
            return Err(PlanError::UnknownSchema);
        }
        registry.validate_type(&self.state_type, budget)?;
        for (index, rule) in self.rules.iter().enumerate() {
            budget.charge(Resource::Work, index as u64 + 1)?;
            if rule.name.is_empty() {
                return Err(PlanError::EmptyName);
            }
            if self.rules[..index].iter().any(|r| r.name == rule.name) {
                return Err(PlanError::DuplicateRule);
            }
            self.expression(rule.root)?;
            registry.validate_type(&rule.output, budget)?;
        }
        for (index, provider) in self.providers.iter().enumerate() {
            budget.charge(Resource::Work, index as u64 + 1)?;
            if self.providers[..index]
                .iter()
                .any(|p| p.operation == provider.operation)
            {
                return Err(PlanError::DuplicateProvider);
            }
            registry.validate_type(&provider.value_input, budget)?;
            registry.validate_type(&provider.value_output, budget)?;
            registry.validate_type(&provider.state_type, budget)?;
            registry.validate_type(&provider.continuation_type, budget)?;
            let descriptor = registry
                .descriptor(&provider.operation.schema)
                .ok_or(PlanError::ProviderSignature)?;
            let operation = descriptor
                .operations
                .iter()
                .find(|op| op.name == provider.operation.name)
                .ok_or(PlanError::ProviderSignature)?;
            let (input, output) = match provider.kind {
                ProviderKind::Read => ("ReadRequest", "ReadReply"),
                ProviderKind::Transform => ("TransformRequest", "TransformReply"),
                ProviderKind::Dependent => ("DependentRequest", "ReadReply"),
            };
            if provider.state_type != self.state_type
                || operation.pure != provider.pure
                || !reader_type(&operation.input, input)
                || !reader_type(&operation.output, output)
                || !reader_type(&provider.continuation_type, "ReaderContinuation")
                || provider.kind == ProviderKind::Read
                    && provider.value_input != TypeDescriptor::Unit
            {
                return Err(PlanError::ProviderSignature);
            }
        }
        for expr in &self.expressions {
            budget.charge(Resource::Nodes, 1)?;
            budget.charge(Resource::Work, 1)?;
            for child in children(expr) {
                self.expression(child)?;
            }
            match expr {
                ReaderExpr::Scalar(CharClass::Range { lo, hi }) if lo > hi => {
                    return Err(PlanError::InvalidRange);
                }
                ReaderExpr::Repeat { min, max, .. } if min > max => {
                    return Err(PlanError::InvalidRepeat);
                }
                ReaderExpr::Until(text) if text.is_empty() => {
                    return Err(PlanError::EmptyDelimiter);
                }
                ReaderExpr::Capture { name, .. } if name.is_empty() => {
                    return Err(PlanError::EmptyName);
                }
                ReaderExpr::Ref(name) => {
                    self.rule(name)?;
                }
                ReaderExpr::Call(operation) => {
                    self.provider(operation, ProviderKind::Read)?;
                }
                ReaderExpr::Decode { provider, .. } => {
                    if !self.provider(provider, ProviderKind::Transform)?.pure {
                        return Err(PlanError::ProviderSignature);
                    }
                }
                ReaderExpr::Map { provider, .. } => {
                    self.provider(provider, ProviderKind::Transform)?;
                }
                ReaderExpr::Then { provider, .. } => {
                    self.provider(provider, ProviderKind::Dependent)?;
                }
                ReaderExpr::Node { kind, .. } => {
                    registry
                        .kind_name(&kind.schema, kind.local_kind)
                        .map_err(|_| PlanError::UnknownKind)?;
                }
                ReaderExpr::Region { class, .. }
                    if class.name.is_empty() || registry.descriptor(&class.schema).is_none() =>
                {
                    return Err(PlanError::UnknownSchema);
                }
                _ => {}
            }
        }
        budget.charge(Resource::AllocationUnits, self.expressions.len() as u64)?;
        let mut nullable = alloc::vec![false;self.expressions.len()];
        loop {
            let mut changed = false;
            for (index, expr) in self.expressions.iter().enumerate() {
                budget.charge(Resource::Work, 1)?;
                let value = self.nullable(expr, &nullable)?;
                if value && !nullable[index] {
                    nullable[index] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for expr in &self.expressions {
            match expr {
                ReaderExpr::Many(body) | ReaderExpr::Some(body) if nullable[body.0 as usize] => {
                    return Err(PlanError::NonProgress);
                }
                ReaderExpr::Repeat { body, max, .. } if *max > 0 && nullable[body.0 as usize] => {
                    return Err(PlanError::NonProgress);
                }
                _ => {}
            }
        }
        self.check_left_recursion(&nullable, budget)?;
        let outputs = self.infer_outputs(budget)?;
        for rule in &self.rules {
            if outputs[rule.root.0 as usize] != rule.output {
                return Err(PlanError::OutputType);
            }
        }
        Ok(CheckedPlan {
            registry,
            plan: self,
            nullable,
            outputs,
        })
    }
    fn nullable(&self, expr: &ReaderExpr, nullable: &[bool]) -> Result<bool, PlanError> {
        Ok(match expr {
            ReaderExpr::Literal(text) => text.is_empty(),
            ReaderExpr::Scalar(_) => false,
            ReaderExpr::Seq(children) => children.iter().all(|id| nullable[id.0 as usize]),
            ReaderExpr::Choice(children) => children.iter().any(|id| nullable[id.0 as usize]),
            ReaderExpr::Many(_)
            | ReaderExpr::Optional(_)
            | ReaderExpr::Look(_)
            | ReaderExpr::Not(_)
            | ReaderExpr::Eof
            | ReaderExpr::Until(_) => true,
            ReaderExpr::Repeat { min, body, .. } => *min == 0 || nullable[body.0 as usize],
            ReaderExpr::Some(body)
            | ReaderExpr::Commit(body)
            | ReaderExpr::Capture { body, .. }
            | ReaderExpr::Region { body, .. }
            | ReaderExpr::Node { body, .. }
            | ReaderExpr::Discard(body)
            | ReaderExpr::Decode { body, .. }
            | ReaderExpr::Map { body, .. } => nullable[body.0 as usize],
            ReaderExpr::Ref(name) => nullable[self.rule(name)?.root.0 as usize],
            ReaderExpr::TakeCount(count) => *count == 0,
            // Programmed providers may return empty matches; runtime progress checking is mandatory.
            ReaderExpr::Call(_) | ReaderExpr::Then { .. } => false,
        })
    }
    fn check_left_recursion(
        &self,
        nullable: &[bool],
        budget: &mut Budget,
    ) -> Result<(), PlanError> {
        budget.charge(Resource::AllocationUnits, self.expressions.len() as u64)?;
        let mut state = alloc::vec![0u8;self.expressions.len()];
        for root in 0..self.expressions.len() {
            let mut pending = Vec::new();
            push(&mut pending, root, false, budget)?;
            while let Some((index, exiting)) = pending.pop() {
                budget.charge(Resource::Work, 1)?;
                if exiting {
                    state[index] = 2;
                    continue;
                }
                match state[index] {
                    1 => return Err(PlanError::LeftRecursion),
                    2 => continue,
                    _ => {}
                }
                state[index] = 1;
                push(&mut pending, index, true, budget)?;
                match &self.expressions[index] {
                    ReaderExpr::Repeat { max: 0, .. } => {}
                    ReaderExpr::Ref(name) => push(
                        &mut pending,
                        self.rule(name)?.root.0 as usize,
                        false,
                        budget,
                    )?,
                    ReaderExpr::Seq(parts) => {
                        for part in parts {
                            push(&mut pending, part.0 as usize, false, budget)?;
                            if !nullable[part.0 as usize] {
                                break;
                            }
                        }
                    }
                    expr => {
                        for child in children(expr) {
                            push(&mut pending, child.0 as usize, false, budget)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn infer_outputs(&self, budget: &mut Budget) -> Result<Vec<TypeDescriptor>, PlanError> {
        use alloc::boxed::Box;
        budget.charge(
            Resource::AllocationUnits,
            (self.expressions.len() as u64)
                .saturating_mul(core::mem::size_of::<Option<TypeDescriptor>>() as u64),
        )?;
        let mut outputs: Vec<Option<TypeDescriptor>> = alloc::vec![None;self.expressions.len()];
        for _ in 0..=self.expressions.len() {
            let mut changed = false;
            for (index, expr) in self.expressions.iter().enumerate() {
                budget.charge(Resource::Work, 1)?;
                if outputs[index].is_some() {
                    continue;
                }
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<TypeDescriptor>() as u64,
                )?;
                let ty = match expr {
                    ReaderExpr::Literal(_)
                    | ReaderExpr::Look(_)
                    | ReaderExpr::Not(_)
                    | ReaderExpr::Discard(_)
                    | ReaderExpr::Eof => Some(TypeDescriptor::Unit),
                    ReaderExpr::Scalar(_) | ReaderExpr::TakeCount(_) | ReaderExpr::Until(_) => {
                        Some(TypeDescriptor::Text)
                    }
                    ReaderExpr::Seq(_) => {
                        Some(TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)))
                    }
                    ReaderExpr::Ref(name) => {
                        Some(self.rule(name)?.output.clone_with_budget(budget)?)
                    }
                    ReaderExpr::Call(op) => Some(
                        self.provider(op, ProviderKind::Read)?
                            .value_output
                            .clone_with_budget(budget)?,
                    ),
                    ReaderExpr::Map { provider, .. } | ReaderExpr::Decode { provider, .. } => Some(
                        self.provider(provider, ProviderKind::Transform)?
                            .value_output
                            .clone_with_budget(budget)?,
                    ),
                    ReaderExpr::Then { provider, .. } => Some(
                        self.provider(provider, ProviderKind::Dependent)?
                            .value_output
                            .clone_with_budget(budget)?,
                    ),
                    ReaderExpr::Choice(parts) => parts
                        .first()
                        .and_then(|id| outputs[id.0 as usize].as_ref())
                        .map(|ty| ty.clone_with_budget(budget))
                        .transpose()?,
                    ReaderExpr::Many(body)
                    | ReaderExpr::Some(body)
                    | ReaderExpr::Repeat { body, .. } => outputs[body.0 as usize]
                        .as_ref()
                        .map(|ty| ty.clone_with_budget(budget))
                        .transpose()?
                        .map(|ty| TypeDescriptor::List(Box::new(ty))),
                    ReaderExpr::Optional(body) => outputs[body.0 as usize]
                        .as_ref()
                        .map(|ty| ty.clone_with_budget(budget))
                        .transpose()?
                        .map(|ty| TypeDescriptor::Option(Box::new(ty))),
                    ReaderExpr::Commit(body)
                    | ReaderExpr::Capture { body, .. }
                    | ReaderExpr::Region { body, .. }
                    | ReaderExpr::Node { body, .. } => outputs[body.0 as usize]
                        .as_ref()
                        .map(|ty| ty.clone_with_budget(budget))
                        .transpose()?,
                };
                if let Some(ty) = ty {
                    outputs[index] = Some(ty);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        budget.charge(
            Resource::AllocationUnits,
            (outputs.len() as u64).saturating_mul(core::mem::size_of::<TypeDescriptor>() as u64),
        )?;
        let outputs: Vec<_> = outputs
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(PlanError::OutputType)?;
        for expr in &self.expressions {
            match expr {
                ReaderExpr::Choice(parts) => {
                    let first = parts.first().ok_or(PlanError::OutputType)?;
                    if parts
                        .iter()
                        .any(|id| outputs[id.0 as usize] != outputs[first.0 as usize])
                    {
                        return Err(PlanError::OutputType);
                    }
                }
                ReaderExpr::Map { provider, body } | ReaderExpr::Decode { provider, body } => {
                    if outputs[body.0 as usize]
                        != self
                            .provider(provider, ProviderKind::Transform)?
                            .value_input
                    {
                        return Err(PlanError::ProviderSignature);
                    }
                }
                ReaderExpr::Then { provider, first }
                    if outputs[first.0 as usize]
                        != self
                            .provider(provider, ProviderKind::Dependent)?
                            .value_input =>
                {
                    return Err(PlanError::ProviderSignature);
                }
                _ => {}
            }
        }
        Ok(outputs)
    }
}
fn children(expr: &ReaderExpr) -> impl Iterator<Item = ReaderId> + '_ {
    let slice: &[ReaderId] = match expr {
        ReaderExpr::Seq(parts) | ReaderExpr::Choice(parts) => parts,
        ReaderExpr::Many(body)
        | ReaderExpr::Some(body)
        | ReaderExpr::Optional(body)
        | ReaderExpr::Look(body)
        | ReaderExpr::Not(body)
        | ReaderExpr::Commit(body)
        | ReaderExpr::Capture { body, .. }
        | ReaderExpr::Region { body, .. }
        | ReaderExpr::Node { body, .. }
        | ReaderExpr::Discard(body)
        | ReaderExpr::Decode { body, .. }
        | ReaderExpr::Map { body, .. }
        | ReaderExpr::Repeat { body, .. } => core::slice::from_ref(body),
        ReaderExpr::Then { first, .. } => core::slice::from_ref(first),
        _ => &[],
    };
    slice.iter().copied()
}
fn push(
    stack: &mut Vec<(usize, bool)>,
    index: usize,
    exiting: bool,
    budget: &mut Budget,
) -> Result<(), PlanError> {
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<(usize, bool)>() as u64,
    )?;
    stack.push((index, exiting));
    Ok(())
}

fn reader_type(ty: &TypeDescriptor, name: &str) -> bool {
    matches!(ty,TypeDescriptor::Named(reference) if reference.package==crate::schema::PACKAGE&&reference.revision==crate::schema::REVISION&&reference.name==name)
}
