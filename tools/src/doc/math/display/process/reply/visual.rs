//! Fixed-resource visual association, not semantic fidelity or Doc admission.
//! Recorded host paths do not qualify executed bootstrap bytes or a portable
//! provider identity. Scope uniqueness and document mounting remain separate.
use super::{Config, Error, Observations, Outcome, PreparedRequest, Reply, Visual};
use crate::doc::math::{
    assets::{BoundProjectedVisual, BoundRenderedVisual, BoundVisual, PreparedAssets},
    katex,
};
use nepl3_core::budget::{Budget, Resource};

#[must_use]
pub enum Preparation<'a, 'r> {
    Visual(AssociatedVisual<'a, 'r>),
    Other(Reply<'a>),
}
/// Immutable native association; no caller can replace its request.
/// ```compile_fail
/// use nepl3_tools::doc::math::display::process::reply::visual::AssociatedVisual;
/// fn replace(v: &mut AssociatedVisual<'_, '_>) { let _ = &mut v.request; }
/// ```
#[must_use]
pub struct AssociatedVisual<'a, 'r> {
    request: &'a PreparedRequest<'a>,
    config: Config<'a>,
    visual: BoundVisual<'r>,
    observations: Option<Observations>,
    termination_failure: bool,
}
/// Serialized content retains its immutable association.
/// ```compile_fail
/// use nepl3_tools::doc::math::display::process::reply::visual::Serialized;
/// fn replace(v: &mut Serialized<'_>) { let _ = &mut v.visual; }
/// ```
#[must_use]
pub struct Serialized<'a> {
    request: &'a PreparedRequest<'a>,
    config: Config<'a>,
    visual: BoundRenderedVisual<'a>,
    observations: Option<&'a Observations>,
    termination_failure: bool,
}
/// Consumed projection retaining the original request/configuration, observed
/// counters and fixed resources. This does not pair an independently supplied
/// Math owner, establish fidelity, or admit a complete accessible artifact.
/// ```compile_fail
/// fn replace(p: &mut nepl3_tools::doc::math::display::process::reply::visual::AssociatedProjectedVisual<'_, '_>) { let _ = &mut p.request; }
/// ```
/// The two input lifetimes are retained independently:
/// ```
/// use nepl3_tools::doc::math::display::process::reply::{Error, visual::{AssociatedVisual, AssociatedProjectedVisual}};
/// fn project<'a, 'r>(v: AssociatedVisual<'a, 'r>, b: &mut nepl3_core::budget::Budget) -> Result<AssociatedProjectedVisual<'a, 'r>, Error> { v.into_html(b) }
/// ```
/// ```compile_fail
/// use nepl3_tools::doc::math::display::process::reply::visual::AssociatedProjectedVisual;
/// fn copy<'a, 'r>(p: &AssociatedProjectedVisual<'a, 'r>) -> AssociatedProjectedVisual<'a, 'r> { p.clone() }
/// ```
/// ```compile_fail
/// fn mutate(p: &nepl3_tools::doc::math::display::process::reply::visual::AssociatedProjectedVisual<'_, '_>) { p.visual().visual().request().fragment.nodes.clear(); }
/// ```
#[must_use]
pub struct AssociatedProjectedVisual<'a, 'r> {
    request: &'a PreparedRequest<'a>,
    config: Config<'a>,
    visual: BoundProjectedVisual<'r>,
    observations: Option<Observations>,
    termination_failure: bool,
}
impl AssociatedProjectedVisual<'_, '_> {
    pub fn request(&self) -> &PreparedRequest<'_> {
        self.request
    }
    pub fn config(&self) -> Config<'_> {
        self.config
    }
    pub fn visual(&self) -> &BoundProjectedVisual<'_> {
        &self.visual
    }
    pub fn observations(&self) -> Option<&Observations> {
        self.observations.as_ref()
    }
    pub fn termination_failure(&self) -> bool {
        self.termination_failure
    }
}
pub(in crate::doc::math::display) fn error(e: katex::Error) -> Error {
    match e {
        katex::Error::Stopped(s) => Error::Stopped(s),
        e => Error::Visual(e),
    }
}
impl<'a> Reply<'a> {
    /// Consumes the actual decoded association. There is no caller-supplied Math
    /// owner, replacement Fragment or renderer-selected class/asset policy.
    /// Nonvisual outcomes retain every observation and are not promoted to fallback.
    pub fn prepare_visual<'r>(
        self,
        assets: &'r PreparedAssets,
        scope: &str,
        b: &mut Budget,
    ) -> Result<Preparation<'a, 'r>, Error> {
        b.poll()?;
        let Self {
            request,
            config,
            outcome,
            observations,
            termination_failure,
        } = self;
        let result = match outcome {
            Outcome::Visual(visual) => prepare_parts(visual, assets, scope, b).map(|visual| {
                Preparation::Visual(AssociatedVisual {
                    request,
                    config,
                    visual,
                    observations,
                    termination_failure,
                })
            }),
            outcome => Ok(Preparation::Other(Reply {
                request,
                config,
                outcome,
                observations,
                termination_failure,
            })),
        };
        b.poll()?;
        result
    }
}
impl<'a, 'r> AssociatedVisual<'a, 'r> {
    /// CSS generation and complete typed validation share the caller budget.
    /// Any stop is an error, never a fallback or partially associated result.
    pub fn into_html(self, b: &mut Budget) -> Result<AssociatedProjectedVisual<'a, 'r>, Error> {
        let visual = self.visual.into_html(b).map_err(error)?;
        Ok(AssociatedProjectedVisual {
            request: self.request,
            config: self.config,
            visual,
            observations: self.observations,
            termination_failure: self.termination_failure,
        })
    }

    pub fn request(&self) -> &PreparedRequest<'_> {
        self.request
    }
    pub fn config(&self) -> Config<'_> {
        self.config
    }
    pub fn visual(&self) -> &BoundVisual<'_> {
        &self.visual
    }
    pub fn observations(&self) -> Option<&Observations> {
        self.observations.as_ref()
    }
    pub fn termination_failure(&self) -> bool {
        self.termination_failure
    }
    /// Revalidates/serializes the admitted finite tree. Raw producer HTML was
    /// discarded; scope uniqueness and document mounting remain host obligations.
    pub fn serialize(&self, b: &mut Budget) -> Result<Serialized<'_>, Error> {
        let visual = self.visual.serialize(b).map_err(error)?;
        Ok(Serialized {
            request: self.request,
            config: self.config,
            visual,
            observations: self.observations.as_ref(),
            termination_failure: self.termination_failure,
        })
    }
}
impl Serialized<'_> {
    pub fn request(&self) -> &PreparedRequest<'_> {
        self.request
    }
    pub fn config(&self) -> Config<'_> {
        self.config
    }
    pub fn visual(&self) -> &BoundRenderedVisual<'_> {
        &self.visual
    }
    pub fn observations(&self) -> Option<&Observations> {
        self.observations
    }
    pub fn termination_failure(&self) -> bool {
        self.termination_failure
    }
}

// Shared by the borrowed public association and the private owning completion
// scope. Neither may omit source-version or fixed-resource validation.
pub(in crate::doc::math::display) fn prepare_parts<'r>(
    visual: Visual,
    assets: &'r PreparedAssets,
    scope: &str,
    b: &mut Budget,
) -> Result<BoundVisual<'r>, Error> {
    b.poll()?;
    b.charge(Resource::Work, assets.version().len() as u64)?;
    if assets.version() != super::catalog::VERSION {
        return Err(Error::Source);
    }
    let result = assets
        .prepare_fragment(visual.fragment, scope, b)
        .map_err(error);
    b.poll()?;
    result
}
