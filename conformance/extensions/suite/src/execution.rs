//! Run language-owned nodes through native Invoke/Await/Resume.
use crate::{
    arithmetic::{self, Application},
    program::{Program, transfer},
};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::{OperationResult, Report},
    operation::{Continuation, Invoke, OperationReply, Resume},
    schema::{SchemaError, SchemaRegistry},
    source::{Digest, SourceError, SourceSnapshot, SourceStore},
    value::{Integer, NdfValue, OperationRef, Record, SchemaRef, TypedValue, Variant},
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
        let environment = transfer::encode(program, &self.operations[0].schema, validation)
            .map_err(Error::Plan)?;
        transfer::validate(
            &environment,
            &self.operations[0].schema,
            registry,
            validation,
        )
        .map_err(Error::Plan)?;
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
        let registrations = [0, 1].map(|i| scheduler::Registration {
            invoke: suspending::Registration {
                operation: &self.operations[i],
                implementation: self.implementations[i],
                invoke: if i == 0 { invoke_mini } else { invoke_frame },
            },
            resume: resume::Registration {
                operation: &self.operations[i],
                implementation: self.implementations[i],
                resume: if i == 0 { resume_mini } else { resume_frame },
            },
            grants: &grants,
            context,
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
fn invalid() -> OperationReply {
    OperationReply::Result(OperationResult::Invalid {
        partial: None,
        report: Report::default(),
    })
}
fn selected(call: &Invoke) -> Option<&Variant> {
    let TypedValue::Record(input) = &call.input else {
        return None;
    };
    let [NdfValue::U64(index)] = input.fields.as_slice() else {
        return None;
    };
    if index.checked_add(1) != Some(call.request_id) {
        return None;
    }
    let TypedValue::Record(environment) = &call.environment else {
        return None;
    };
    let [NdfValue::List(nodes)] = environment.fields.as_slice() else {
        return None;
    };
    let NdfValue::Variant(node) = nodes.get(usize::try_from(*index).ok()?)? else {
        return None;
    };
    Some(node)
}
// Every run has a fresh lifetime table and immutable, exact environment/source
// grants. No continuation or context cache crosses run boundaries. Within this
// scope, the configured executable identity distinguishes the two providers.
fn context(_: &Invoke, implementation: Digest, b: &mut Budget) -> Result<Digest, StopReason> {
    b.poll()?;
    Ok(implementation)
}

fn invoke_mini(
    call: &Invoke,
    context: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    invoke(call, context, false, b)
}
fn invoke_frame(
    call: &Invoke,
    context: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    invoke(call, context, true, b)
}
fn invoke(
    call: &Invoke,
    context: Digest,
    frame: bool,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 1)?;
    let Some(node) = selected(call) else {
        return Ok(invalid());
    };
    if (node.variant == "Frame") != frame {
        return Ok(invalid());
    }
    if let ("Natural", [NdfValue::Integer(value)]) = (node.variant.as_str(), node.fields.as_slice())
    {
        let value = NdfValue::Integer(copy_integer(value, b)?);
        return complete(&call.operation.schema, value, b);
    }
    let mut calls = Vec::new();
    for child in &node.fields {
        let NdfValue::U64(index) = child else {
            return Ok(invalid());
        };
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Invoke>() as u64,
        )?;
        calls
            .try_reserve_exact(1)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        let mut operation = copy_operation(&call.operation, b)?;
        b.charge(Resource::AllocationUnits, 8)?;
        operation.name = if node.variant == "Framed" {
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
                [NdfValue::U64(*index)],
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
        report: Report::default(),
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
        report: Report::default(),
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
fn resume_mini(
    call: &Invoke,
    reply: &Resume,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    resume_value(call, reply, false, b)
}
fn resume_frame(
    call: &Invoke,
    reply: &Resume,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    resume_value(call, reply, true, b)
}
fn resume_value(
    call: &Invoke,
    reply: &Resume,
    frame: bool,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.poll()?;
    let Some(node) = selected(call) else {
        return Ok(invalid());
    };
    if (node.variant == "Frame") != frame {
        return Ok(invalid());
    }
    let children = reply.dependency_results.as_slice();
    let value = match (node.variant.as_str(), children) {
        ("Frame" | "Framed", [child]) => {
            let Some(value) = number(child) else {
                return Ok(invalid());
            };
            copy_integer(value, b)?
        }
        ("Neg", [child]) => {
            let Some(value) = number(child) else {
                return Ok(invalid());
            };
            arithmetic::apply(Application::Neg(value), b)?
        }
        ("Add" | "Mul", [left, right]) => {
            let (Some(left), Some(right)) = (number(left), number(right)) else {
                return Ok(invalid());
            };
            arithmetic::apply(
                if node.variant == "Add" {
                    Application::Add(left, right)
                } else {
                    Application::Mul(left, right)
                },
                b,
            )?
        }
        _ => return Ok(invalid()),
    };
    complete(&call.operation.schema, NdfValue::Integer(value), b)
}
