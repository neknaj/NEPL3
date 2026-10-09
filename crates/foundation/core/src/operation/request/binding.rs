//! Metered equality against the exact Invoke retained by a host.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestBindingError {
    Stopped(StopReason),
    RequestId,
    Operation,
    Input,
    Environment,
    Sources,
    Resources,
    Limits,
}
impl From<StopReason> for RequestBindingError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl Invoke {
    /// Compare the complete request representation, including ordered source and
    /// resource tables. This is not schema validation, authorization, a semantic
    /// context digest, measurement authentication or execution-attempt identity.
    /// Equal copies from different attempts cannot be distinguished here.
    /// Successful comparison charges remain on mismatch or resource stop.
    pub fn check_saved(&self, saved: &Self, b: &mut Budget) -> Result<(), RequestBindingError> {
        b.charge(Resource::Work, 1)?;
        if self.request_id != saved.request_id {
            return Err(RequestBindingError::RequestId);
        }
        b.charge(
            Resource::Work,
            (self.operation.schema.package.len() as u64)
                .saturating_add(saved.operation.schema.package.len() as u64)
                .saturating_add(self.operation.name.len() as u64)
                .saturating_add(saved.operation.name.len() as u64)
                .saturating_add(41),
        )?;
        if self.operation != saved.operation {
            return Err(RequestBindingError::Operation);
        }
        b.charge(Resource::Work, 8)?;
        if self.limits != saved.limits {
            return Err(RequestBindingError::Limits);
        }
        if !self.input.equal_with_budget(&saved.input, b)? {
            return Err(RequestBindingError::Input);
        }
        if !self.environment.equal_with_budget(&saved.environment, b)? {
            return Err(RequestBindingError::Environment);
        }
        b.charge(Resource::Work, 1)?;
        if self.sources.len() != saved.sources.len() {
            return Err(RequestBindingError::Sources);
        }
        for (a, c) in self.sources.iter().zip(&saved.sources) {
            b.charge(Resource::Work, 1)?;
            if !a.eq_with_budget(c, b)? {
                return Err(RequestBindingError::Sources);
            }
        }
        b.charge(Resource::Work, 1)?;
        if self.resources.len() != saved.resources.len() {
            return Err(RequestBindingError::Resources);
        }
        for (a, c) in self.resources.iter().zip(&saved.resources) {
            b.charge(
                Resource::Work,
                (a.id.len() as u64)
                    .saturating_add(c.id.len() as u64)
                    .saturating_add(a.bytes.len() as u64)
                    .saturating_add(c.bytes.len() as u64)
                    .saturating_add(33),
            )?;
            if a != c {
                return Err(RequestBindingError::Resources);
            }
        }
        Ok(())
    }
}
