//! In-memory page preparation checkpoints. Prepared is not cross-page
//! validation, rendering, publication, or persistence across process exit.
//! Setup, retained host containers and I/O are outside measured phase usage.
use super::{
    Attempt, Cancelled, Limits, Meter, Paused, Running, StopReason, Usage, covers, remaining,
};
use crate::doc::{
    export::pages::{Entry, code::PreparedPageCode, resources::OperationLimits},
    source::{Compiled, ParseObservationOutcome, err, with_named_input_limits_observed},
};
use nepl3_core::{
    budget::Budget,
    source::{Digest, SourceAdmission, SourceStore},
};
use nepl3_doc_core::{check::Category, lower, model::DocumentSyntax};
use nepl3_wire::foundation::FoundationCodec;

/// Independent per-page preparation allowances, additionally bounded by the
/// cumulative job ledger. Code preparation does not include PageSet rendering.
#[derive(Clone, Copy, Debug, Default)]
pub struct PreparationLimits {
    pub parse_limits: OperationLimits,
    pub lower_limits: OperationLimits,
    pub code_limits: OperationLimits,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Parse,
    Lower,
    Code,
}
#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub stage: Stage,
    pub requested: Limits,
    pub effective: Limits,
    pub usage: Usage,
    pub stop: Option<StopReason>,
    pub failed: bool,
    pub accounting_rejected: bool,
    pub profile: Option<Digest>,
}
#[derive(Debug)]
pub struct PageAttempt {
    pub page_index: usize,
    pub stages: Vec<Observation>,
}
struct Page {
    document: DocumentSyntax,
    // Keep owned Code evidence inseparable from the exact prepared page.
    _code: PreparedPageCode,
}
struct Data<'a> {
    compiled: &'a Compiled,
    inputs: Vec<(Entry, String)>,
    phases: PreparationLimits,
    prepared: Vec<Page>,
    attempts: Vec<PageAttempt>,
}
pub struct Preparation<'a> {
    data: Data<'a>,
    account: Running,
}
pub struct PausedPreparation<'a> {
    data: Data<'a>,
    account: Paused,
}
pub struct RejectedResume<'a> {
    paused: PausedPreparation<'a>,
}
pub struct PreparedPages<'a> {
    data: Data<'a>,
    usage: Usage,
}
pub struct CancelledPreparation {
    pub accounting: Cancelled,
    pub attempts: Vec<PageAttempt>,
}
pub enum Progress<'a> {
    Prepared(PreparedPages<'a>),
    Paused {
        job: PausedPreparation<'a>,
        error: Option<String>,
    },
    Failed {
        error: String,
        usage: Usage,
        attempts: Vec<PageAttempt>,
    },
    Cancelled(CancelledPreparation),
}
impl<'a> Preparation<'a> {
    pub fn new(
        compiled: &'a Compiled,
        inputs: Vec<(Entry, String)>,
        limits: Limits,
        phases: PreparationLimits,
    ) -> Result<Self, String> {
        if inputs.is_empty() || inputs.len() > 128 {
            return Err("PageCountLimit".into());
        }
        let mut bytes = 0u64;
        for (entry, input) in &inputs {
            crate::doc::export::pages::input_path(entry)?;
            bytes = bytes.checked_add(input.len() as u64).ok_or("SourceLimit")?;
            if bytes > crate::doc::export::MAX_SOURCE_BYTES {
                return Err("SourceLimit".into());
            }
        }
        let mut prepared = Vec::new();
        prepared
            .try_reserve_exact(inputs.len())
            .map_err(|_| "HostAllocationFailure")?;
        Ok(Self {
            data: Data {
                compiled,
                inputs,
                phases,
                prepared,
                attempts: Vec::new(),
            },
            account: Running::new(limits),
        })
    }
    pub fn run(self) -> Progress<'a> {
        let Self {
            mut data,
            mut account,
        } = self;
        while data.prepared.len() < data.inputs.len() {
            if data.attempts.try_reserve(1).is_err() {
                return Progress::Failed {
                    error: "HostAllocationFailure".into(),
                    usage: account.usage(),
                    attempts: data.attempts,
                };
            }
            let page_index = data.prepared.len();
            let mut stages = Vec::new();
            if stages.try_reserve_exact(3).is_err() {
                return Progress::Failed {
                    error: "HostAllocationFailure".into(),
                    usage: account.usage(),
                    attempts: data.attempts,
                };
            }
            let (entry, source) = &data.inputs[page_index];
            let cap = account.limits();
            let result = account.run(cap, |meter| {
                prepare(
                    meter,
                    data.compiled,
                    entry,
                    source,
                    data.phases,
                    &mut stages,
                )
            });
            data.attempts.push(PageAttempt { page_index, stages });
            match result {
                Attempt::Complete { value, running } => {
                    data.prepared.push(value);
                    account = running;
                }
                Attempt::Paused { paused, error } => {
                    if data
                        .attempts
                        .last()
                        .is_some_and(|a| a.stages.iter().any(|s| s.accounting_rejected))
                    {
                        return Progress::Failed {
                            error: error.unwrap_or_default(),
                            usage: paused.usage(),
                            attempts: data.attempts,
                        };
                    }
                    return Progress::Paused {
                        job: PausedPreparation {
                            data,
                            account: paused,
                        },
                        error,
                    };
                }
                Attempt::Failed { error, usage, .. } => {
                    return Progress::Failed {
                        error,
                        usage,
                        attempts: data.attempts,
                    };
                }
                Attempt::Cancelled(cancelled) => {
                    return Progress::Cancelled(CancelledPreparation {
                        accounting: cancelled,
                        attempts: data.attempts,
                    });
                }
            }
        }
        Progress::Prepared(PreparedPages {
            data,
            usage: account.usage(),
        })
    }
    pub fn cancel(self) -> CancelledPreparation {
        CancelledPreparation {
            accounting: self.account.cancel(),
            attempts: self.data.attempts,
        }
    }
}
impl<'a> PausedPreparation<'a> {
    pub fn prepared_count(&self) -> usize {
        self.data.prepared.len()
    }
    pub fn usage(&self) -> Usage {
        self.account.usage()
    }
    pub fn reason(&self) -> StopReason {
        self.account.reason()
    }
    pub fn attempts(&self) -> &[PageAttempt] {
        &self.data.attempts
    }
    pub fn resume(
        self,
        limits: Limits,
        phases: PreparationLimits,
    ) -> Result<Preparation<'a>, Box<RejectedResume<'a>>> {
        if !phase_covers(phases, self.data.phases) || !covers(limits, self.account.limits()) {
            return Err(Box::new(RejectedResume { paused: self }));
        }
        let Self { mut data, account } = self;
        match account.resume(limits) {
            Ok(account) => {
                data.phases = phases;
                Ok(Preparation { data, account })
            }
            Err(rejected) => Err(Box::new(RejectedResume {
                paused: Self {
                    data,
                    account: rejected.into_paused(),
                },
            })),
        }
    }
    pub fn cancel(self) -> CancelledPreparation {
        CancelledPreparation {
            accounting: self.account.cancel(),
            attempts: self.data.attempts,
        }
    }
}
impl<'a> RejectedResume<'a> {
    pub fn into_paused(self) -> PausedPreparation<'a> {
        self.paused
    }
}
impl PreparedPages<'_> {
    pub fn prepared_count(&self) -> usize {
        self.data.prepared.len()
    }
    pub fn usage(&self) -> Usage {
        self.usage
    }
    pub fn attempts(&self) -> &[PageAttempt] {
        &self.data.attempts
    }
    pub fn documents(&self) -> impl Iterator<Item = &DocumentSyntax> {
        self.data.prepared.iter().map(|p| &p.document)
    }
}
fn phase_covers(a: PreparationLimits, b: PreparationLimits) -> bool {
    covers(
        a.parse_limits.budget().limits(),
        b.parse_limits.budget().limits(),
    ) && covers(
        a.lower_limits.budget().limits(),
        b.lower_limits.budget().limits(),
    ) && covers(
        a.code_limits.budget().limits(),
        b.code_limits.budget().limits(),
    )
}
fn grant(meter: &Meter<'_>, requested: Limits) -> Result<Limits, String> {
    meter.poll().map_err(err)?;
    Ok(remaining(
        meter.budget.limits(),
        meter.budget.usage(),
        requested,
    ))
}
fn settle(
    meter: &mut Meter<'_>,
    mut record: Observation,
    records: &mut Vec<Observation>,
) -> Result<(), String> {
    // Grants and settlement are sequential. Rejection is an invariant failure,
    // never permission to discard usage and continue with another phase.
    let settlement = meter.budget.record_observed_usage(record.usage);
    record.accounting_rejected = settlement.is_err();
    records.push(record);
    settlement.map_err(|reason| format!("PhaseAccountingInvariant: {reason:?}"))?;
    if let Some(reason) = record.stop {
        meter.budget.stop(reason);
        return Err(format!("{:?}: {reason:?}", record.stage));
    }
    Ok(())
}
fn stage<T>(
    meter: &mut Meter<'_>,
    kind: Stage,
    requested: Limits,
    profile: Digest,
    records: &mut Vec<Observation>,
    operation: impl FnOnce(&mut Budget) -> Result<T, String>,
) -> Result<T, String> {
    let effective = grant(meter, requested)?;
    let mut budget = Budget::new(effective);
    let result = operation(&mut budget);
    settle(
        meter,
        Observation {
            stage: kind,
            requested,
            effective,
            usage: budget.usage(),
            stop: budget.poll().err(),
            failed: result.is_err(),
            accounting_rejected: false,
            profile: Some(profile),
        },
        records,
    )?;
    result.map_err(|error| format!("{kind:?}: {error}"))
}
fn prepare(
    meter: &mut Meter<'_>,
    compiled: &Compiled,
    entry: &Entry,
    input: &str,
    phases: PreparationLimits,
    records: &mut Vec<Observation>,
) -> Result<Page, String> {
    let requested = phases.parse_limits.budget().limits();
    let effective = grant(meter, requested)?;
    let mut entered = false;
    let mut terminal = None;
    let result = with_named_input_limits_observed(
        true,
        compiled,
        input,
        &entry.id,
        "Article",
        effective,
        |observation| terminal = Some(observation),
        |tree, profile, parsing, _admission| {
            entered = true;
            settle(
                meter,
                Observation {
                    stage: Stage::Parse,
                    requested,
                    effective,
                    usage: parsing.usage(),
                    stop: parsing.poll().err(),
                    failed: false,
                    accounting_rejected: false,
                    profile: Some(profile.digest()),
                },
                records,
            )?;
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let document = stage(
                meter,
                Stage::Lower,
                phases.lower_limits.budget().limits(),
                profile.digest(),
                records,
                |budget| {
                    lower::document(
                        tree.syntax(),
                        &compiled.doc.package.schema,
                        Category::Article,
                        profile.registry(),
                        budget,
                        &mut codec,
                    )
                    .map_err(err)
                },
            )?;
            let code = stage(
                meter,
                Stage::Code,
                phases.code_limits.budget().limits(),
                profile.digest(),
                records,
                |budget| PreparedPageCode::prepare(&document, tree, profile, budget),
            )?;
            Ok(Page {
                document,
                _code: code,
            })
        },
    );
    if !entered {
        let observation = terminal.ok_or("MissingParseObservation")?;
        let stop = match observation.outcome {
            ParseObservationOutcome::Stopped(reason) => Some(reason),
            _ => None,
        };
        settle(
            meter,
            Observation {
                stage: Stage::Parse,
                requested,
                effective,
                usage: observation.usage,
                stop,
                failed: true,
                accounting_rejected: false,
                profile: observation.profile,
            },
            records,
        )?;
    }
    result
}

#[cfg(test)]
mod tests;
