//! Concrete plan identity for continuation/cache matching; Grammar semantic normalization is separate.
use super::{CharClass, ProviderKind, ReaderExpr, ReaderId, ReaderPlan};
use alloc::{string::ToString, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{CanonicalWriter, SchemaError},
    source::Digest,
    value::{OperationRef, SchemaRef},
    view::FallbackRole,
};

impl ReaderPlan {
    pub fn canonical_json(&self, budget: &mut Budget) -> Result<Vec<u8>, SchemaError> {
        budget.charge(
            Resource::AllocationUnits,
            ((self.rules.len() + self.providers.len()) as u64)
                .saturating_mul(core::mem::size_of::<usize>() as u64),
        )?;
        let mut rules: Vec<_> = self.rules.iter().collect();
        rules.sort_by(|a, b| a.name.cmp(&b.name));
        let mut providers: Vec<_> = self.providers.iter().collect();
        providers.sort_by(|a, b| {
            (&a.operation.schema, &a.operation.name).cmp(&(&b.operation.schema, &b.operation.name))
        });
        if rules.windows(2).any(|p| p[0].name == p[1].name)
            || providers
                .windows(2)
                .any(|p| p[0].operation == p[1].operation)
        {
            return Err(SchemaError::DuplicateName);
        }
        let mut out = CanonicalWriter::new(budget);
        out.push("{\"expressions\":[")?;
        for (index, expr) in self.expressions.iter().enumerate() {
            if index != 0 {
                out.push(",")?;
            }
            expression(&mut out, expr)?;
        }
        out.push("],\"providers\":[")?;
        for (index, provider) in providers.iter().enumerate() {
            if index != 0 {
                out.push(",")?;
            }
            out.push("{\"continuationType\":")?;
            out.ty(&provider.continuation_type, 0)?;
            out.push(",\"kind\":")?;
            out.quoted(match provider.kind {
                ProviderKind::Read => "Read",
                ProviderKind::Transform => "Transform",
                ProviderKind::Dependent => "Dependent",
            })?;
            out.push(",\"operation\":")?;
            operation(&mut out, &provider.operation)?;
            out.push(",\"pure\":")?;
            out.push(if provider.pure { "true" } else { "false" })?;
            out.push(",\"stateType\":")?;
            out.ty(&provider.state_type, 0)?;
            out.push(",\"valueInput\":")?;
            out.ty(&provider.value_input, 0)?;
            out.push(",\"valueOutput\":")?;
            out.ty(&provider.value_output, 0)?;
            out.push("}")?;
        }
        out.push("],\"rules\":{")?;
        for (index, rule) in rules.iter().enumerate() {
            if index != 0 {
                out.push(",")?;
            }
            out.quoted(&rule.name)?;
            out.push(":{\"output\":")?;
            out.ty(&rule.output, 0)?;
            out.push(",\"root\":")?;
            out.number(rule.root.0)?;
            out.push("}")?;
        }
        out.push("},\"schema\":")?;
        schema(&mut out, &self.schema)?;
        out.push(",\"stateType\":")?;
        out.ty(&self.state_type, 0)?;
        out.push("}")?;
        Ok(out.finish())
    }
    pub fn digest(&self, budget: &mut Budget) -> Result<Digest, SchemaError> {
        Ok(Digest::domain(
            b"NEPL3-READER-1\0",
            &self.canonical_json(budget)?,
        ))
    }
}
fn schema(out: &mut CanonicalWriter<'_>, schema: &SchemaRef) -> Result<(), SchemaError> {
    out.push("{\"digest\":[")?;
    for (i, b) in schema.digest.0.iter().enumerate() {
        if i != 0 {
            out.push(",")?;
        }
        out.number(u64::from(*b))?;
    }
    out.push("],\"package\":")?;
    out.quoted(&schema.package)?;
    out.push(",\"revision\":")?;
    out.number(schema.revision)?;
    out.push("}")
}
fn operation(out: &mut CanonicalWriter<'_>, operation: &OperationRef) -> Result<(), SchemaError> {
    out.push("{\"name\":")?;
    out.quoted(&operation.name)?;
    out.push(",\"schema\":")?;
    schema(out, &operation.schema)?;
    out.push("}")
}
fn ids(out: &mut CanonicalWriter<'_>, ids: &[ReaderId]) -> Result<(), SchemaError> {
    out.push("[")?;
    for (index, id) in ids.iter().enumerate() {
        if index != 0 {
            out.push(",")?;
        }
        out.number(id.0)?;
    }
    out.push("]")
}
fn class(out: &mut CanonicalWriter<'_>, class: &CharClass) -> Result<(), SchemaError> {
    out.push("[")?;
    match class {
        CharClass::Any => out.quoted("Any")?,
        CharClass::Whitespace => out.quoted("Whitespace")?,
        CharClass::IdentifierStart => out.quoted("IdentifierStart")?,
        CharClass::IdentifierContinue => out.quoted("IdentifierContinue")?,
        CharClass::Digit => out.quoted("Digit")?,
        CharClass::AsciiLetter => out.quoted("AsciiLetter")?,
        CharClass::Chars(text) | CharClass::Except(text) => {
            out.quoted(if matches!(class, CharClass::Chars(_)) {
                "Chars"
            } else {
                "Except"
            })?;
            out.push(",")?;
            out.quoted(text)?;
        }
        CharClass::Range { lo, hi } => {
            out.quoted("Range")?;
            out.push(",")?;
            out.quoted(&lo.to_string())?;
            out.push(",")?;
            out.quoted(&hi.to_string())?;
        }
    }
    out.push("]")
}
fn expression(out: &mut CanonicalWriter<'_>, expr: &ReaderExpr) -> Result<(), SchemaError> {
    out.push("[")?;
    match expr {
        ReaderExpr::Literal(text) | ReaderExpr::Until(text) | ReaderExpr::Ref(text) => {
            out.quoted(match expr {
                ReaderExpr::Literal(_) => "Literal",
                ReaderExpr::Until(_) => "Until",
                _ => "Ref",
            })?;
            out.push(",")?;
            out.quoted(text)?;
        }
        ReaderExpr::Scalar(value) => {
            out.quoted("Scalar")?;
            out.push(",")?;
            class(out, value)?;
        }
        ReaderExpr::Seq(parts) | ReaderExpr::Choice(parts) => {
            out.quoted(if matches!(expr, ReaderExpr::Seq(_)) {
                "Seq"
            } else {
                "Choice"
            })?;
            out.push(",")?;
            ids(out, parts)?;
        }
        ReaderExpr::Many(body)
        | ReaderExpr::Some(body)
        | ReaderExpr::Optional(body)
        | ReaderExpr::Look(body)
        | ReaderExpr::Not(body)
        | ReaderExpr::Commit(body)
        | ReaderExpr::Discard(body) => {
            out.quoted(match expr {
                ReaderExpr::Many(_) => "Many",
                ReaderExpr::Some(_) => "Some",
                ReaderExpr::Optional(_) => "Optional",
                ReaderExpr::Look(_) => "Look",
                ReaderExpr::Not(_) => "Not",
                ReaderExpr::Commit(_) => "Commit",
                _ => "Discard",
            })?;
            out.push(",")?;
            out.number(body.0)?;
        }
        ReaderExpr::Repeat { min, max, body } => {
            out.quoted("Repeat")?;
            out.push(",")?;
            out.number(*min)?;
            out.push(",")?;
            out.number(*max)?;
            out.push(",")?;
            out.number(body.0)?;
        }
        ReaderExpr::Capture { name, body } => {
            out.quoted("Capture")?;
            out.push(",")?;
            out.quoted(name)?;
            out.push(",")?;
            out.number(body.0)?;
        }
        ReaderExpr::Region { class, body } => {
            out.quoted("Region")?;
            out.push(",{\"fallback\":")?;
            out.quoted(match class.fallback {
                FallbackRole::Content => "Content",
                FallbackRole::Marker => "Marker",
                FallbackRole::Delimiter => "Delimiter",
                FallbackRole::Name => "Name",
                FallbackRole::Quantity => "Quantity",
                FallbackRole::Annotation => "Annotation",
            })?;
            out.push(",\"name\":")?;
            out.quoted(&class.name)?;
            out.push(",\"schema\":")?;
            schema(out, &class.schema)?;
            out.push("},")?;
            out.number(body.0)?;
        }
        ReaderExpr::Node { kind, body } => {
            out.quoted("Node")?;
            out.push(",{\"localKind\":")?;
            out.number(kind.local_kind)?;
            out.push(",\"schema\":")?;
            schema(out, &kind.schema)?;
            out.push("},")?;
            out.number(body.0)?;
        }
        ReaderExpr::Decode { provider, body }
        | ReaderExpr::Map { provider, body }
        | ReaderExpr::Then {
            provider,
            first: body,
        } => {
            out.quoted(match expr {
                ReaderExpr::Decode { .. } => "Decode",
                ReaderExpr::Map { .. } => "Map",
                _ => "Then",
            })?;
            out.push(",")?;
            operation(out, provider)?;
            out.push(",")?;
            out.number(body.0)?;
        }
        ReaderExpr::Call(provider) => {
            out.quoted("Call")?;
            out.push(",")?;
            operation(out, provider)?;
        }
        ReaderExpr::Eof => out.quoted("Eof")?,
        ReaderExpr::TakeCount(count) => {
            out.quoted("TakeCount")?;
            out.push(",")?;
            out.number(*count)?;
        }
    }
    out.push("]")
}
