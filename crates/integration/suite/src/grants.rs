//! Exact host-approved invocation context, independent of wire-supplied identity.
pub mod dependencies;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    operation::Invoke,
    source::SourceStore,
    syntax::{ResourceContent, SyntaxError, validate_resources},
    value::TypedValue,
};

#[derive(Debug)]
pub enum GrantError {
    Stopped(StopReason),
    InvalidResources(SyntaxError),
    Environment,
    Source,
    Resource,
}
impl From<StopReason> for GrantError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
fn resources(error: SyntaxError) -> GrantError {
    match error {
        SyntaxError::Stopped(s) => GrantError::Stopped(s),
        e => GrantError::InvalidResources(e),
    }
}

/// Immutable authority prepared by the host after policy/environment projection.
/// Environment must match this exact projection. Sources and resources may be
/// subsets of the grants. Resource integrity/uniqueness is checked once on entry.
/// Operation selection and schema validation remain separate dispatch checks.
pub struct Grants<'a> {
    environment: &'a TypedValue,
    sources: &'a SourceStore,
    resources: &'a [ResourceContent],
}

/// Borrowed proof of context permission. Both the invocation and its grants stay
/// immutable for this lifetime. This is not a schema or provider capability proof.
pub struct AuthorizedInvoke<'a> {
    request: &'a Invoke,
    _grants: &'a Grants<'a>,
}
impl AuthorizedInvoke<'_> {
    pub fn request(&self) -> &Invoke {
        self.request
    }
}

impl<'a> Grants<'a> {
    pub fn new(
        environment: &'a TypedValue,
        sources: &'a SourceStore,
        granted_resources: &'a [ResourceContent],
        budget: &mut Budget,
    ) -> Result<Self, GrantError> {
        validate_resources(granted_resources, budget).map_err(resources)?;
        Ok(Self {
            environment,
            sources,
            resources: granted_resources,
        })
    }

    /// Uses the SourceStore index and budgeted immutable snapshot equality.
    /// Resource matching scans the granted list; all comparisons and resource
    /// integrity checks are charged. No source admission or grant mutation occurs.
    pub fn admit<'b>(
        &'b self,
        request: &'b Invoke,
        budget: &mut Budget,
    ) -> Result<AuthorizedInvoke<'b>, GrantError> {
        if !request
            .environment
            .equal_with_budget(self.environment, budget)?
        {
            return Err(GrantError::Environment);
        }
        for source in &request.sources {
            let identity = source.identity();
            let granted = self
                .sources
                .get_revision_with_budget(&identity.source, identity.revision, budget)?
                .ok_or(GrantError::Source)?;
            if !source.eq_with_budget(granted, budget)? {
                return Err(GrantError::Source);
            }
        }
        validate_resources(&request.resources, budget).map_err(resources)?;
        for resource in &request.resources {
            let mut matched = None;
            for grant in self.resources {
                budget.charge(
                    Resource::Work,
                    (resource.id.len() as u64)
                        .saturating_add(grant.id.len() as u64)
                        .saturating_add(1),
                )?;
                if resource.id == grant.id {
                    matched = Some(grant);
                    break;
                }
            }
            let grant = matched.ok_or(GrantError::Resource)?;
            budget.charge(Resource::Work, 32)?;
            if resource.digest != grant.digest {
                return Err(GrantError::Resource);
            }
            budget.charge(
                Resource::Work,
                (resource.bytes.len() as u64).saturating_add(grant.bytes.len() as u64),
            )?;
            if resource.bytes != grant.bytes {
                return Err(GrantError::Resource);
            }
        }
        Ok(AuthorizedInvoke {
            request,
            _grants: self,
        })
    }
}
