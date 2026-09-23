//! Run language-owned nodes through native Invoke/Await/Resume.
#[cfg(test)]
mod tests;
use crate::{
    arithmetic::{self, Application},
    program::{Instruction, Node, Program, transfer},
    syntax::Language,
};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::{Diagnostic, OperationResult, Report, Severity},
    operation::{Continuation, Invoke, OperationReply, Resume},
    schema::{SchemaError, SchemaRegistry},
    source::{Digest, SourceError, SourceSnapshot, SourceStore, Span},
    value::{Integer, NdfValue, OperationRef, Record, SchemaRef, TypedValue},
};
use nepl3_suite::{
    dispatch::{resume, suspending},
    grants::{GrantError, Grants},
    scheduler,
};

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Schema(SchemaError),
    Plan(transfer::Error),
    Source(SourceError),
    Grants(GrantError),
    Context(nepl3_wire::WireError),
    Execution(scheduler::Failure),
}
impl From<StopReason> for Error {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}

/// Host-selected executable identities. The registration callbacks are private;
/// run admits one immutable plan and one source closure before exposing them.
pub struct Runtime {
    operations: [OperationRef; 2],
    implementations: [Digest; 2],
}
impl Runtime {
    pub fn register(
        registry: &mut SchemaRegistry,
        implementations: [Digest; 2],
        b: &mut Budget,
    ) -> Result<Self, Error> {
        let foundation = nepl3_core::schema::foundation::descriptor(b).map_err(Error::Schema)?;
        let foundation_ref = foundation.reference(b).map_err(Error::Schema)?;
        if registry.descriptor(&foundation_ref).is_none() {
            registry
                .register(foundation_ref, foundation, b)
                .map_err(Error::Schema)?;
        }
        let descriptor = transfer::descriptor(b).map_err(Error::Stopped)?;
        let schema = descriptor.reference(b).map_err(Error::Schema)?;
        b.charge(Resource::AllocationUnits, 1024)?;
        let operations = ["miniexpr", "frame"].map(|name| OperationRef {
            schema: schema.clone(),
            name: name.into(),
        });
        registry
            .register(schema, descriptor, b)
            .map_err(Error::Schema)?;
        Ok(Self {
            operations,
            implementations,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        program: &Program<'_>,
        sources: &SourceStore,
        registry: &SchemaRegistry,
        execution: &mut Budget,
        validation: &mut Budget,
        report: impl FnMut(u64, Report),
        cancel: impl FnMut(u64),
    ) -> Result<OperationResult<TypedValue>, Error> {
        for node in program.nodes() {
            validation.charge(Resource::Work, 1)?;
            if let Some(span) = node.head {
                let identity = span.snapshot_ref();
                let source = sources
                    .get_revision_with_budget(&identity.source, identity.revision, validation)?
                    .ok_or(Error::Source(SourceError::MissingSnapshot))?;
                source.slice(span).map_err(Error::Source)?;
            }
        }
        let encoded_plan = transfer::encode(program, &self.operations[0].schema, validation)
            .map_err(Error::Plan)?;
        transfer::validate(
            &encoded_plan,
            &self.operations[0].schema,
            registry,
            validation,
        )
        .map_err(Error::Plan)?;
        let digest = plan_digest(&encoded_plan, validation)?;
        validation.charge(Resource::AllocationUnits, 32)?;
        let environment = record(
            &self.operations[0].schema,
            "PlanIdentity",
            [NdfValue::Bytes(digest.0.to_vec())],
            validation,
        )?;
        let root_index =
            u64::try_from(program.root().0).map_err(|_| validation.stop(StopReason::NodeLimit))?;
        let root = Invoke {
            request_id: root_index
                .checked_add(1)
                .ok_or_else(|| validation.stop(StopReason::NodeLimit))?,
            operation: copy_operation(&self.operations[0], validation)?,
            input: record(
                &self.operations[0].schema,
                "Selection",
                [NdfValue::U64(root_index)],
                validation,
            )?,
            environment: environment.clone_with_budget(validation)?,
            sources: copy_sources(sources.snapshots(), validation)?,
            resources: vec![],
            limits: execution.limits(),
        };
        let grants = Grants::new(&environment, sources, &[], validation).map_err(Error::Grants)?;
        grants.admit(&root, validation).map_err(Error::Grants)?;
        let contexts = [
            nepl3_wire::operation::context_digest(
                &root,
                self.implementations[0],
                registry,
                validation,
            )
            .map_err(Error::Context)?,
            nepl3_wire::operation::context_digest(
                &root,
                self.implementations[1],
                registry,
                validation,
            )
            .map_err(Error::Context)?,
        ];
        // Each callback copies this exact admitted environment and complete
        // source closure. The immutable digest is shared for this run only.
        let mini_context = |_: &Invoke, _: Digest, b: &mut Budget| {
            b.poll()?;
            Ok(contexts[0])
        };
        let frame_context = |_: &Invoke, _: Digest, b: &mut Budget| {
            b.poll()?;
            Ok(contexts[1])
        };
        let context_callbacks: [&scheduler::Context<'_>; 2] = [&mini_context, &frame_context];
        let mini_invoke = |call: &Invoke, context: Digest, _: &SchemaRegistry, b: &mut Budget| {
            invoke(program, call, context, false, b)
        };
        let frame_invoke = |call: &Invoke, context: Digest, _: &SchemaRegistry, b: &mut Budget| {
            invoke(program, call, context, true, b)
        };
        let mini_resume = |call: &Invoke, reply: &Resume, _: &SchemaRegistry, b: &mut Budget| {
            resume_value(program, call, reply, false, b)
        };
        let frame_resume = |call: &Invoke, reply: &Resume, _: &SchemaRegistry, b: &mut Budget| {
            resume_value(program, call, reply, true, b)
        };
        let invoke_callbacks: [&suspending::Callback<'_>; 2] = [&mini_invoke, &frame_invoke];
        let resume_callbacks: [&resume::Callback<'_>; 2] = [&mini_resume, &frame_resume];
        let registrations = [0, 1].map(|i| scheduler::Registration {
            invoke: suspending::Registration {
                operation: &self.operations[i],
                implementation: self.implementations[i],
                invoke: invoke_callbacks[i],
            },
            resume: resume::Registration {
                operation: &self.operations[i],
                implementation: self.implementations[i],
                resume: resume_callbacks[i],
            },
            grants: &grants,
            context: context_callbacks[i],
        });
        scheduler::run(
            &registrations,
            &root,
            registry,
            execution,
            validation,
            report,
            cancel,
        )
        .map_err(Error::Execution)
    }
}

fn plan_digest(value: &TypedValue, budget: &mut Budget) -> Result<Digest, Error> {
    let value = match value.clone_with_budget(budget)? {
        TypedValue::Record(record) => NdfValue::Record(record),
        TypedValue::Variant(variant) => NdfValue::Variant(variant),
    };
    let bytes = nepl3_wire::encode(&value, budget).map_err(Error::Context)?;
    budget.charge(Resource::Work, bytes.len() as u64)?;
    Ok(Digest::domain(b"nepl3.example.composition-plan/1", &bytes))
}

fn copy_operation(op: &OperationRef, b: &mut Budget) -> Result<OperationRef, StopReason> {
    let bytes =
        (op.schema.package.len() + op.name.len() + core::mem::size_of::<OperationRef>()) as u64;
    b.charge(Resource::Work, bytes)?;
    b.charge(Resource::AllocationUnits, bytes)?;
    Ok(op.clone())
}
fn copy_sources(
    sources: &[SourceSnapshot],
    b: &mut Budget,
) -> Result<Vec<SourceSnapshot>, StopReason> {
    let mut output = Vec::new();
    let bytes = sources
        .len()
        .checked_mul(core::mem::size_of::<SourceSnapshot>())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    output
        .try_reserve_exact(sources.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    for source in sources {
        output.push(source.clone_with_budget(b)?);
    }
    Ok(output)
}
fn record<const N: usize>(
    schema: &SchemaRef,
    kind: &str,
    fields: [NdfValue; N],
    b: &mut Budget,
) -> Result<TypedValue, StopReason> {
    let bytes = (schema.package.len()
        + kind.len()
        + core::mem::size_of::<Record>()
        + fields.len() * core::mem::size_of::<NdfValue>()) as u64;
    b.charge(Resource::Work, bytes)?;
    b.charge(Resource::AllocationUnits, bytes)?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(N)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    owned.extend(fields);
    Ok(TypedValue::Record(Record {
        schema: schema.clone(),
        kind: kind.into(),
        fields: owned,
    }))
}
fn invalid(b: &Budget) -> OperationReply {
    OperationReply::Result(OperationResult::Invalid {
        partial: None,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
fn selected<'a, 'source>(
    program: &'a Program<'source>,
    call: &Invoke,
) -> Option<&'a Node<'source>> {
    let TypedValue::Record(input) = &call.input else {
        return None;
    };
    let [NdfValue::U64(index)] = input.fields.as_slice() else {
        return None;
    };
    if index.checked_add(1) != Some(call.request_id) {
        return None;
    }
    program.request_node(call.request_id)
}
fn invoke(
    program: &Program<'_>,
    call: &Invoke,
    context: Digest,
    frame: bool,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 1)?;
    let Some(node) = selected(program, call) else {
        return Ok(invalid(b));
    };
    if (node.language == Language::Frame) != frame {
        return Ok(invalid(b));
    }
    let (left, right) = match node.instruction {
        Instruction::Natural(value) => {
            return complete(
                &call.operation.schema,
                NdfValue::Integer(copy_integer(value, b)?),
                b,
            );
        }
        Instruction::Neg(child) | Instruction::Framed(child) | Instruction::Frame(child) => {
            (child, None)
        }
        Instruction::Add(left, right) | Instruction::Mul(left, right) => (left, Some(right)),
    };
    let mut calls = Vec::new();
    for child in core::iter::once(left).chain(right) {
        let index = u64::try_from(child.0).map_err(|_| b.stop(StopReason::NodeLimit))?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Invoke>() as u64,
        )?;
        calls
            .try_reserve_exact(1)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        let mut operation = copy_operation(&call.operation, b)?;
        b.charge(Resource::AllocationUnits, 8)?;
        operation.name = if matches!(node.instruction, Instruction::Framed(_)) {
            "frame"
        } else {
            "miniexpr"
        }
        .into();
        calls.push(Invoke {
            request_id: index
                .checked_add(1)
                .ok_or_else(|| b.stop(StopReason::NodeLimit))?,
            operation,
            input: record(
                &call.operation.schema,
                "Selection",
                [NdfValue::U64(index)],
                b,
            )?,
            environment: call.environment.clone_with_budget(b)?,
            sources: copy_sources(&call.sources, b)?,
            resources: vec![],
            limits: call.limits,
        });
    }
    Ok(OperationReply::Await {
        continuation: Continuation {
            provider: copy_operation(&call.operation, b)?,
            parent_request: call.request_id,
            snapshot_digest: context,
            state: call.input.clone_with_budget(b)?,
        },
        calls,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
fn copy_integer(value: &Integer, b: &mut Budget) -> Result<Integer, StopReason> {
    let bytes = value.as_bigint().bits() / 8 + 1;
    b.charge(Resource::Work, bytes)?;
    b.charge(Resource::AllocationUnits, bytes.saturating_add(32))?;
    Ok(value.clone())
}
fn complete(
    schema: &SchemaRef,
    value: NdfValue,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    Ok(OperationReply::Result(OperationResult::Complete {
        value: record(schema, "Value", [value], b)?,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    }))
}
fn number(reply: &OperationReply) -> Option<&Integer> {
    let OperationReply::Result(OperationResult::Complete {
        value: TypedValue::Record(value),
        ..
    }) = reply
    else {
        return None;
    };
    let [NdfValue::Integer(value)] = value.fields.as_slice() else {
        return None;
    };
    Some(value)
}
fn resume_value(
    program: &Program<'_>,
    call: &Invoke,
    reply: &Resume,
    frame: bool,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.poll()?;
    let Some(node) = selected(program, call) else {
        return Ok(invalid(b));
    };
    if (node.language == Language::Frame) != frame {
        return Ok(invalid(b));
    }
    let children = reply.dependency_results.as_slice();
    let application = match (&node.instruction, children) {
        (Instruction::Frame(_) | Instruction::Framed(_), [child]) => {
            let Some(value) = number(child) else {
                return Ok(invalid(b));
            };
            return complete(
                &call.operation.schema,
                NdfValue::Integer(copy_integer(value, b)?),
                b,
            );
        }
        (Instruction::Neg(_), [child]) => {
            let Some(value) = number(child) else {
                return Ok(invalid(b));
            };
            Application::Neg(value)
        }
        (Instruction::Add(..) | Instruction::Mul(..), [left, right]) => {
            let (Some(left), Some(right)) = (number(left), number(right)) else {
                return Ok(invalid(b));
            };
            if matches!(node.instruction, Instruction::Add(..)) {
                Application::Add(left, right)
            } else {
                Application::Mul(left, right)
            }
        }
        _ => return Ok(invalid(b)),
    };
    // Reserve the owned diagnostic before arithmetic can exhaust the Budget.
    // Failure during preparation remains a host failure with active source
    // context. Once prepared, returning Stopped requires no further allocation.
    let mut report = prepare_stop_report(call, node.head, b)?;
    match arithmetic::apply(application, b)
        .and_then(|value| complete(&call.operation.schema, NdfValue::Integer(value), b))
    {
        Ok(result) => Ok(result),
        Err(reason) => {
            report.usage = b.usage();
            Ok(OperationReply::Result(OperationResult::Stopped {
                reason,
                partial: None,
                report,
            }))
        }
    }
}

fn prepare_stop_report(
    call: &Invoke,
    span: Option<&Span>,
    b: &mut Budget,
) -> Result<Report, StopReason> {
    const CODE: &str = "evaluation-stopped";
    const STAGE: &str = "evaluate";
    let source_bytes = span.map_or(0, |span| span.snapshot_ref().source.0.len());
    let bytes = (core::mem::size_of::<Diagnostic>()
        + call.operation.schema.package.len()
        + CODE.len()
        + STAGE.len()
        + source_bytes) as u64;
    b.charge(Resource::Work, bytes)?;
    b.charge(Resource::AllocationUnits, bytes)?;
    b.charge(Resource::Diagnostics, 1)?;
    let mut diagnostics = Vec::new();
    diagnostics
        .try_reserve_exact(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    let arguments = call.input.clone_with_budget(b)?;
    diagnostics.push(Diagnostic {
        schema: call.operation.schema.clone(),
        code: CODE.into(),
        severity: Severity::Error,
        stage: STAGE.into(),
        arguments,
        primary: span.cloned(),
        related: vec![],
        fixes: vec![],
    });
    Ok(Report {
        diagnostics,
        usage: b.usage(),
        ..Report::default()
    })
}
