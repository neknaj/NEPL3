use super::*;
use crate::selection::HeadProviderRef;
impl<'a> ResolvedParseProfile<'a> {
    /// The registration belongs to the explicit alias/category, not just to the
    /// surface schema: two aliases of one package may select different providers.
    pub fn head_provider(
        &self,
        alias: &str,
        category: &str,
        budget: &mut Budget,
    ) -> Result<Option<&'a HeadProviderRef>, ProfileError> {
        let package = self.language(alias, budget)?;
        lookup(budget, package.categories.len(), category.len())?;
        package
            .category(category)
            .map_err(|_| ProfileError::MissingCategory)?;
        lookup(
            budget,
            self.profile.head_providers.len(),
            alias.len() + category.len(),
        )?;
        Ok(self
            .profile
            .head_providers
            .iter()
            .find(|v| v.alias == alias && v.category == category)
            .map(|v| &v.provider))
    }
    pub(super) fn validate_heads(&self, budget: &mut Budget) -> Result<(), ProfileError> {
        for (i, entry) in self.profile.head_providers.iter().enumerate() {
            lookup(budget, i, entry.alias.len() + entry.category.len())?;
            if self.profile.head_providers[..i]
                .iter()
                .any(|v| v.alias == entry.alias && v.category == entry.category)
            {
                return Err(ProfileError::Duplicate);
            }
            self.head_provider(&entry.alias, &entry.category, budget)?;
            for operation in [&entry.provider.shape, &entry.provider.child_context] {
                self.provider(operation, budget)?;
                let descriptor = self
                    .registry
                    .descriptor(&operation.schema)
                    .ok_or(ProfileError::MissingSchema)?;
                lookup(budget, descriptor.operations.len(), operation.name.len())?;
                let signature = descriptor
                    .operations
                    .iter()
                    .find(|v| v.name == operation.name)
                    .ok_or(ProfileError::MissingProvider)?;
                // Expected named-type strings are fixed, bounded protocol names.
                budget.charge(Resource::Work, 128)?;
                if !crate::head::signature(&signature.input, &signature.output, signature.pure) {
                    return Err(ProfileError::HeadSignature);
                }
            }
        }
        Ok(())
    }
}
