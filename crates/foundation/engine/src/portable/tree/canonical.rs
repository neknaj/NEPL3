use super::*;
use nepl3_core::syntax::{SyntaxBundle, canonical::NodeMapping};

pub(crate) struct Mappings<'a> {
    pub entries: Vec<NodeMapping<'a>>,
}
impl<'a> Mappings<'a> {
    pub fn new<E>(bundle: &'a SyntaxBundle, b: &mut Budget) -> Result<Self, PortableError<E>> {
        let entries = nepl3_core::syntax::canonical::BundleMappings::new(bundle, b)?.into_entries();
        Ok(Self { entries })
    }
    pub fn owner<E>(
        &self,
        bundle: &SyntaxBundle,
        b: &mut Budget,
    ) -> Result<&NodeMapping<'a>, PortableError<E>> {
        for entry in &self.entries {
            b.charge(Resource::Work, 1)?;
            if core::ptr::eq(entry.bundle(), bundle) {
                return Ok(entry);
            }
        }
        Err(PortableError::Shape)
    }
    pub fn path_value<C: FoundationValueCodec>(
        &self,
        mut bundle: &'a SyntaxBundle,
        path: &[ForeignStep],
        s: &Schemas<'_>,
        profile: &ResolvedParseProfile<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<(NdfValue, &'a SyntaxBundle), PortableError<C::Error>> {
        let mut values = Vec::new();
        for step in path {
            let mapped = self.owner(bundle, b)?.mapped(step.node)?;
            push(
                &mut values,
                record(
                    s.engine,
                    "ForeignStep",
                    [mapped.value(s, c, b)?, step.field.value(s, c, b)?],
                    b,
                )?,
                b,
            )?;
            bundle = crate::tree::path(bundle, core::slice::from_ref(step), profile.registry(), b)?;
        }
        Ok((NdfValue::List(values), bundle))
    }
    pub fn check_tables<E>(
        &self,
        tree: &ParseTree,
        profile: &ResolvedParseProfile<'_>,
        b: &mut Budget,
    ) -> Result<(), PortableError<E>> {
        if tree.contexts.len() != self.entries.len() {
            return Err(PortableError::NonCanonical);
        }
        for (context, mapping) in tree.contexts.iter().zip(&self.entries) {
            if !core::ptr::eq(
                crate::tree::path(&tree.bundle, &context.path, profile.registry(), b)?,
                mapping.bundle(),
            ) {
                return Err(PortableError::NonCanonical);
            }
            for (i, node) in context.nodes.iter().enumerate() {
                b.charge(Resource::Work, 1)?;
                if node.node.0 != i as u64 {
                    return Err(PortableError::NonCanonical);
                }
            }
        }
        let mut previous = None;
        for recovery in &tree.recovery {
            let bundle = crate::tree::path(&tree.bundle, &recovery.path, profile.registry(), b)?;
            let mut at = None;
            for (i, entry) in self.entries.iter().enumerate() {
                b.charge(Resource::Work, 1)?;
                if core::ptr::eq(entry.bundle(), bundle) {
                    at = Some(i);
                    break;
                }
            }
            let at = at.ok_or(PortableError::Shape)?;
            if previous.is_some_and(|v| v >= at) {
                return Err(PortableError::NonCanonical);
            }
            previous = Some(at);
            let mut prior = None;
            for entry in &recovery.entries {
                b.charge(Resource::Work, 1)?;
                if prior.is_some_and(|id| id >= entry.node.0) {
                    return Err(PortableError::NonCanonical);
                }
                prior = Some(entry.node.0);
            }
        }
        Ok(())
    }
}
