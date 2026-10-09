//! Native completion scope retaining the original owned Math preparation.
//! The driver returns decoded attempt data, not an arbitrary visual tree.
//! This neither drives process cleanup nor selects a fallback/artifact policy.
use super::{
    PreparedDisplay,
    process::{
        Config,
        reply::{self, Observations, Outcome, Reply},
    },
    request::{self, Controls, PreparedRequest},
};
use crate::doc::math::assets::{BoundProjectedVisual, PreparedAssets};
use nepl3_core::budget::{Budget, Resource, StopReason};

pub struct Setup<'c, 'r, 's> {
    pub controls: Controls,
    pub request_cap: usize,
    pub config: Config<'c>,
    pub assets: &'r PreparedAssets,
    pub scope: &'s str,
}
#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
}
impl From<StopReason> for Error {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mismatch {
    Request,
    Config,
}
/// Attempt outcomes, not successful document/artifact outcomes. Other never
/// contains Visual; rejected/failed attempts cannot authorize visual admission.
pub enum Attempt<'r, E> {
    NotRequested,
    RequestRejected(request::Error),
    DriverFailed(E),
    AssociationRejected(Mismatch),
    ValidationFailed(reply::Error),
    Other(Outcome),
    Visual(BoundProjectedVisual<'r>),
}
/// Original source/MathML, copied scope name and owned completion data, with no temporary request
/// borrow. Selected config/controls are not a qualified execution identity.
/// ```compile_fail
/// fn replace<E>(g: &mut nepl3_tools::doc::math::display::generation::OwnedGeneration<'_, '_, E>) { let _ = &mut g.prepared; }
/// ```
/// ```compile_fail
/// fn mutate<E>(g: &OwnedGeneration<'_, '_, E>) { g.prepared().mathml().syntax.value.nodes.clear(); }
/// use nepl3_tools::doc::math::display::generation::OwnedGeneration;
/// ```
/// ```compile_fail
/// fn extract<E>(g: nepl3_tools::doc::math::display::generation::OwnedGeneration<'_, '_, E>) { g.into_parts(); }
/// ```
/// ```compile_fail
/// use nepl3_tools::doc::math::display::{PreparedDisplay, generation::Setup, process::reply::Reply};
/// fn escape(p: PreparedDisplay, setup: Setup<'_, '_, '_>, b: &mut nepl3_core::budget::Budget) {
///     let mut escaped = None;
///     let _ = p.generate_owned(setup, |request, _, _| { escaped = Some(request); Err::<Reply<'_>, ()>(()) }, b);
/// }
/// ```
pub struct OwnedGeneration<'c, 'r, E> {
    prepared: PreparedDisplay,
    config: Config<'c>,
    controls: Controls,
    scope: String,
    attempt: Attempt<'r, E>,
    observations: Option<Observations>,
    termination_failure: Option<bool>,
}
impl<'c, E> OwnedGeneration<'c, '_, E> {
    pub fn prepared(&self) -> &PreparedDisplay {
        &self.prepared
    }
    pub fn selected_config(&self) -> Config<'c> {
        self.config
    }
    pub fn controls(&self) -> Controls {
        self.controls
    }
    pub fn selected_scope(&self) -> &str {
        &self.scope
    }
    pub fn attempt(&self) -> &Attempt<'_, E> {
        &self.attempt
    }
    pub fn observations(&self) -> Option<&Observations> {
        self.observations.as_ref()
    }
    pub fn termination_failure(&self) -> Option<bool> {
        self.termination_failure
    }
}
fn same_config(a: Config<'_>, c: Config<'_>, b: &mut Budget) -> Result<bool, StopReason> {
    let mut work = 8u64;
    for n in [
        a.node.as_os_str().len(),
        a.bridge.as_os_str().len(),
        a.modules_url.len(),
        c.node.as_os_str().len(),
        c.bridge.as_os_str().len(),
        c.modules_url.len(),
    ] {
        work = work
            .checked_add(n as u64)
            .ok_or_else(|| b.stop(StopReason::WorkLimit))?;
    }
    b.charge(Resource::Work, work)?;
    Ok(a.node.as_os_str() == c.node.as_os_str()
        && a.bridge.as_os_str() == c.bridge.as_os_str()
        && a.modules_url == c.modules_url
        && a.input_cap == c.input_cap
        && a.output_cap == c.output_cap
        && a.timeout == c.timeout)
}
fn own_scope(scope: &str, b: &mut Budget) -> Result<String, StopReason> {
    b.poll()?;
    b.charge(Resource::Work, scope.len() as u64)?;
    b.charge(Resource::AllocationUnits, scope.len() as u64)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(scope.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    owned.push_str(scope);
    Ok(owned)
}
fn project_outcome<'r, E>(
    outcome: Outcome,
    assets: &'r PreparedAssets,
    scope: &str,
    b: &mut Budget,
) -> Result<Attempt<'r, E>, Error> {
    Ok(match outcome {
        Outcome::Visual(visual) => {
            let result = reply::visual::prepare_parts(visual, assets, scope, b)
                .and_then(|visual| visual.into_html(b).map_err(reply::visual::error));
            b.poll()?;
            match result {
                Ok(visual) => Attempt::Visual(visual),
                Err(error) => Attempt::ValidationFailed(error),
            }
        }
        other => Attempt::Other(other),
    })
}
/// Native transport exit stays alongside generation data rather than being
/// silently discarded. Neither field is a CLI/artifact success certificate.
/// ```compile_fail
/// use nepl3_tools::doc::math::display::generation::NativeGeneration;
/// fn replace(a: &mut NativeGeneration<'_, '_>, b: NativeGeneration<'_, '_>) { a.generation = b.generation; }
/// ```
#[must_use]
pub struct NativeGeneration<'c, 'r> {
    generation: OwnedGeneration<'c, 'r, super::process::driver::DriverError>,
    exit: std::process::ExitStatus,
}
impl<'c, 'r> NativeGeneration<'c, 'r> {
    pub fn generation(&self) -> &OwnedGeneration<'c, 'r, super::process::driver::DriverError> {
        &self.generation
    }
    pub fn exit(&self) -> std::process::ExitStatus {
        self.exit
    }
    pub fn into_parts(
        self,
    ) -> (
        OwnedGeneration<'c, 'r, super::process::driver::DriverError>,
        std::process::ExitStatus,
    ) {
        (self.generation, self.exit)
    }
}
impl<'c> super::process::owned::Decoded<'c> {
    /// Continue an actually completed sealed native launch using its original
    /// cumulative Budget. This selects only post-decode assets/scope, never a
    /// replacement source, launch configuration or controls. Existing completion
    /// policy and artifact admission are still required afterward.
    pub fn into_generation<'r>(
        self,
        assets: &'r PreparedAssets,
        scope: &str,
        b: &mut Budget,
    ) -> Result<NativeGeneration<'c, 'r>, Error> {
        b.poll()?;
        let scope = own_scope(scope, b)?;
        let (prepared, controls, config, result, exit) = self.into_parts();
        let (attempt, observations, termination_failure) = match result {
            Err(error) => (
                Attempt::DriverFailed(super::process::driver::DriverError::Decode(error)),
                None,
                None,
            ),
            Ok((outcome, observations, termination_failure)) => (
                project_outcome(outcome, assets, &scope, b)?,
                observations,
                Some(termination_failure),
            ),
        };
        b.poll()?;
        Ok(NativeGeneration {
            generation: OwnedGeneration {
                prepared,
                config,
                controls,
                scope,
                attempt,
                observations,
                termination_failure,
            },
            exit,
        })
    }
}
impl<'c> super::process::owned::Failed<'c> {
    /// Preserve a terminal transport failure and its exact original preparation.
    /// Use the original cumulative execution Budget, never a fresh ledger.
    /// This does not turn failed transport into an ordinary MathML fallback.
    pub fn into_generation<'r>(
        self,
        scope: &str,
        b: &mut Budget,
    ) -> Result<NativeGeneration<'c, 'r>, Error> {
        b.poll()?;
        let scope = own_scope(scope, b)?;
        let (prepared, controls, config, failure, exit) = self.into_parts();
        b.poll()?;
        Ok(NativeGeneration {
            generation: OwnedGeneration {
                prepared,
                config,
                controls,
                scope,
                attempt: Attempt::DriverFailed(super::process::driver::DriverError::Transport(
                    failure,
                )),
                observations: None,
                termination_failure: None,
            },
            exit,
        })
    }
}
impl PreparedDisplay {
    /// Invoke the native driver at most once inside this owner's borrow scope.
    /// The driver must use the supplied cumulative Budget honestly; an &mut
    /// reference cannot prevent a trusted Rust callback replacing that ledger.
    /// Driver failure is retained as attempt data with original MathML. A parent
    /// stop always returns Err, including when the callback returns Ok or Err.
    /// Request-preparation rejection is retained explicitly and calls no driver;
    /// only a genuine None becomes NotRequested. Neither is an artifact result.
    pub fn generate_owned<'c, 'r, E, F>(
        self,
        setup: Setup<'c, 'r, '_>,
        driver: F,
        b: &mut Budget,
    ) -> Result<OwnedGeneration<'c, 'r, E>, Error>
    where
        F: for<'a> FnOnce(&'a PreparedRequest<'a>, Config<'a>, &mut Budget) -> Result<Reply<'a>, E>,
    {
        let scope = own_scope(setup.scope, b)?;
        let request = request::owned::prepare(self, setup.controls, setup.request_cap, b);
        b.poll()?;
        let (prepared, (attempt, observations, termination_failure)) = match request {
            request::owned::Preparation::Rejected {
                error: request::Error::Stopped(s),
                ..
            } => return Err(Error::Stopped(s)),
            request::owned::Preparation::Rejected { owner, error } => {
                (owner, (Attempt::RequestRejected(error), None, None))
            }
            request::owned::Preparation::NotRequested(owner) => {
                (owner, (Attempt::NotRequested, None, None))
            }
            request::owned::Preparation::Ready(request) => {
                let (owner, outcome) = request.consume(|request| -> Result<_, Error> {
                    let result = driver(request, setup.config, b);
                    b.poll()?;
                    let outcome = match result {
                        Err(error) => (Attempt::DriverFailed(error), None, None),
                        Ok(reply) => {
                            b.charge(Resource::Work, 1)?;
                            if !core::ptr::eq(reply.request(), request) {
                                (Attempt::AssociationRejected(Mismatch::Request), None, None)
                            } else if !same_config(reply.config(), setup.config, b)? {
                                (Attempt::AssociationRejected(Mismatch::Config), None, None)
                            } else {
                                let (outcome, observations, termination_failure) =
                                    reply.into_owned_data();
                                let attempt = project_outcome(outcome, setup.assets, &scope, b)?;
                                (attempt, observations, Some(termination_failure))
                            }
                        }
                    };
                    Ok(outcome)
                });
                (owner, outcome?)
            }
        };
        b.poll()?;
        Ok(OwnedGeneration {
            prepared,
            config: setup.config,
            controls: setup.controls,
            scope,
            attempt,
            observations,
            termination_failure,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_core::budget::Limits;
    use std::{path::Path, time::Duration};
    #[test]
    fn owned_scope_copy_is_prepaid_and_independent() -> Result<(), StopReason> {
        let mut input = String::with_capacity(1024);
        input.push_str("nepl-math-transient");
        let n = input.len() as u64;
        let mut exact = Budget::new(Limits {
            work: n,
            allocation_units: n,
            ..Limits::default()
        });
        let owned = own_scope(&input, &mut exact)?;
        assert_eq!(owned, input);
        assert_ne!(owned.as_ptr(), input.as_ptr());
        input.clear();
        drop(input);
        assert_eq!(owned, "nepl-math-transient");
        assert_eq!(exact.usage().work, n);
        assert_eq!(exact.usage().allocation_units, n);
        assert_eq!(exact.usage().output_bytes, 0);
        for allocation in [false, true] {
            let mut limited = Budget::new(Limits {
                work: n - u64::from(!allocation),
                allocation_units: n - u64::from(allocation),
                ..Limits::default()
            });
            let expected = if allocation {
                StopReason::AllocationLimit
            } else {
                StopReason::WorkLimit
            };
            assert!(matches!(own_scope(&owned, &mut limited), Err(s) if s == expected));
            let before = limited.usage();
            assert!(matches!(own_scope(&owned, &mut limited), Err(s) if s == expected));
            assert_eq!(limited.usage(), before);
        }
        let mut canceled = Budget::new(Limits {
            work: n,
            allocation_units: n,
            ..Limits::default()
        });
        canceled.cancel();
        assert_eq!(own_scope(&owned, &mut canceled), Err(StopReason::Cancelled));
        Ok(())
    }
    #[test]
    fn complete_config_comparison_is_byte_exact_and_prepaid() -> Result<(), StopReason> {
        let base = Config {
            node: Path::new("bin/node"),
            bridge: Path::new("dir/main.js"),
            modules_url: "file:///modules/",
            input_cap: 4096,
            output_cap: 100000,
            timeout: Duration::from_millis(5000),
        };
        for index in 0..6 {
            let mut other = base;
            match index {
                0 => {
                    other.node = Path::new("bin/./node");
                    assert_eq!(base.node, other.node);
                }
                1 => {
                    other.bridge = Path::new("dir/./main.js");
                    assert_eq!(base.bridge, other.bridge);
                }
                2 => other.modules_url = "file:///other/",
                3 => other.input_cap += 1,
                4 => other.output_cap += 1,
                _ => other.timeout += Duration::from_nanos(1),
            }
            let mut b = Budget::new(Limits {
                work: 1000,
                ..Limits::default()
            });
            assert!(!same_config(base, other, &mut b)?);
            let work = b.usage().work;
            let mut exact = Budget::new(Limits {
                work,
                ..Limits::default()
            });
            assert!(!same_config(base, other, &mut exact)?);
            let mut short = Budget::new(Limits {
                work: work - 1,
                ..Limits::default()
            });
            assert_eq!(
                same_config(base, other, &mut short),
                Err(StopReason::WorkLimit)
            );
            let usage = short.usage();
            short.cancel();
            assert_eq!(
                same_config(base, other, &mut short),
                Err(StopReason::WorkLimit)
            );
            assert_eq!(short.usage(), usage);
            assert_eq!(b.usage().allocation_units, 0);
            assert_eq!(b.usage().output_bytes, 0);
        }
        assert!(same_config(
            base,
            base,
            &mut Budget::new(Limits {
                work: 1000,
                ..Limits::default()
            })
        )?);
        Ok(())
    }
}

pub mod composite;
