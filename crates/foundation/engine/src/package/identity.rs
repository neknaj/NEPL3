//! Semantic identity excludes provenance and arena allocation, preserving operation order.
use super::*;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::CanonicalWriter,
    source::Digest,
    value::{KindRef, OperationRef, SchemaRef},
};
use nepl3_reader::{builtin::BuiltinReader, tokenizer::TokenReader};

mod execution;
mod reader;

pub(crate) fn sorted<'a, T>(
    items: &'a [T],
    key: impl Fn(&T) -> (&str, &str),
    budget: &mut Budget,
) -> Result<Vec<&'a T>, PackageError> {
    budget.charge(
        Resource::AllocationUnits,
        (items.len() * core::mem::size_of::<&T>()) as u64,
    )?;
    let mut ordered = Vec::with_capacity(items.len());
    for item in items {
        let target = key(item);
        let mut index = ordered.len();
        while index > 0 {
            let prior = key(ordered[index - 1]);
            budget.charge(
                Resource::Work,
                (target.0.len().min(prior.0.len()) + target.1.len().min(prior.1.len())) as u64 + 1,
            )?;
            if prior <= target {
                break;
            }
            index -= 1;
        }
        budget.charge(Resource::Work, (ordered.len() - index) as u64 + 1)?;
        ordered.insert(index, item);
    }
    Ok(ordered)
}
pub(crate) fn schema(out: &mut CanonicalWriter<'_>, value: &SchemaRef) -> Result<(), PackageError> {
    out.push("{\"digest\":[")?;
    for (i, b) in value.digest.0.iter().enumerate() {
        comma(out, i)?;
        out.number(u64::from(*b))?;
    }
    out.push("],\"package\":")?;
    out.quoted(&value.package)?;
    out.push(",\"revision\":")?;
    out.number(value.revision)?;
    out.push("}")?;
    Ok(())
}
fn kind(out: &mut CanonicalWriter<'_>, value: &KindRef) -> Result<(), PackageError> {
    out.push("[")?;
    schema(out, &value.schema)?;
    out.push(",")?;
    out.number(value.local_kind)?;
    out.push("]")?;
    Ok(())
}
pub(crate) fn operation(
    out: &mut CanonicalWriter<'_>,
    value: &OperationRef,
) -> Result<(), PackageError> {
    out.push("[")?;
    schema(out, &value.schema)?;
    out.push(",")?;
    out.quoted(&value.name)?;
    out.push("]")?;
    Ok(())
}
fn comma(out: &mut CanonicalWriter<'_>, index: usize) -> Result<(), PackageError> {
    if index != 0 {
        out.push(",")?;
    }
    Ok(())
}
fn builtin(value: BuiltinReader) -> &'static str {
    match value {
        BuiltinReader::Name => "Name",
        BuiltinReader::Nat => "Nat",
        BuiltinReader::Number => "Number",
        BuiltinReader::Text => "Text",
        BuiltinReader::Lang => "Lang",
        BuiltinReader::Trivia => "Trivia",
    }
}
fn token_reader(out: &mut CanonicalWriter<'_>, value: &TokenReader) -> Result<(), PackageError> {
    match value {
        TokenReader::Builtin(v) => {
            out.push("[\"Builtin\",")?;
            out.quoted(builtin(*v))?;
        }
        TokenReader::Rule(v) => {
            out.push("[\"Rule\",")?;
            out.quoted(v)?;
        }
    }
    out.push("]")?;
    Ok(())
}
fn name(out: &mut CanonicalWriter<'_>, value: &NameSelector) -> Result<(), PackageError> {
    match value {
        NameSelector::SelfValue => out.push("[\"SelfValue\"]")?,
        NameSelector::Field(v) => {
            out.push("[\"Field\",")?;
            out.quoted(v)?;
            out.push("]")?;
        }
    }
    Ok(())
}
fn styles(out: &mut CanonicalWriter<'_>, values: &[StyleRule]) -> Result<(), PackageError> {
    out.push("[")?;
    for (i, value) in values.iter().enumerate() {
        comma(out, i)?;
        out.push("[")?;
        match &value.selector {
            StyleSelector::Head => out.push("[\"Head\"]")?,
            StyleSelector::SelfValue => out.push("[\"SelfValue\"]")?,
            StyleSelector::Field(v) | StyleSelector::Capture(v) => {
                out.push(if matches!(value.selector, StyleSelector::Field(_)) {
                    "[\"Field\","
                } else {
                    "[\"Capture\","
                })?;
                out.quoted(v)?;
                out.push("]")?;
            }
        }
        out.push(",")?;
        schema(out, &value.class.schema)?;
        out.push(",")?;
        out.quoted(&value.class.name)?;
        out.push(",")?;
        out.quoted(match value.class.fallback {
            nepl3_core::view::FallbackRole::Content => "Content",
            nepl3_core::view::FallbackRole::Marker => "Marker",
            nepl3_core::view::FallbackRole::Delimiter => "Delimiter",
            nepl3_core::view::FallbackRole::Name => "Name",
            nepl3_core::view::FallbackRole::Quantity => "Quantity",
            nepl3_core::view::FallbackRole::Annotation => "Annotation",
        })?;
        out.push("]")?;
    }
    out.push("]")?;
    Ok(())
}
fn read(
    out: &mut CanonicalWriter<'_>,
    package: &LanguagePackage,
    mut id: ReadSpecId,
) -> Result<(), PackageError> {
    let mut depth = 0u64;
    loop {
        depth = depth
            .checked_add(1)
            .ok_or(nepl3_core::budget::StopReason::DepthLimit)?;
        out.budget().observe_depth(depth)?;
        match package.read(id)? {
            ReadSpec::Builtin {
                reader,
                kind: k,
                token_kind,
            } => {
                out.push("[\"Builtin\",")?;
                out.quoted(builtin(*reader))?;
                out.push(",")?;
                kind(out, k)?;
                out.push(",")?;
                kind(out, token_kind)?;
                break;
            }
            ReadSpec::Local { category } => {
                out.push("[\"Local\",")?;
                out.quoted(category)?;
                break;
            }
            ReadSpec::Foreign { alias, category } => {
                out.push("[\"Foreign\",")?;
                out.quoted(alias)?;
                out.push(",")?;
                out.quoted(category)?;
                break;
            }
            ReadSpec::WithMode { mode, read } => {
                out.push("[\"WithMode\",")?;
                out.quoted(mode)?;
                out.push(",")?;
                id = *read;
            }
            ReadSpec::ListOf { element, cons, nil } => {
                out.push("[\"ListOf\",")?;
                kind(out, cons)?;
                out.push(",")?;
                kind(out, nil)?;
                out.push(",")?;
                id = *element;
            }
        }
    }
    for _ in 0..depth {
        out.push("]")?;
    }
    Ok(())
}
fn binding(
    out: &mut CanonicalWriter<'_>,
    package: &LanguagePackage,
    root: BindingId,
) -> Result<(), PackageError> {
    enum Action {
        Node(BindingId, u64),
        Text(&'static str),
    }
    fn push(
        v: &mut Vec<Action>,
        a: Action,
        out: &mut CanonicalWriter<'_>,
    ) -> Result<(), PackageError> {
        out.budget().charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Action>() as u64,
        )?;
        v.push(a);
        Ok(())
    }
    let mut pending = Vec::new();
    push(&mut pending, Action::Node(root, 1), out)?;
    while let Some(action) = pending.pop() {
        out.budget().charge(Resource::Work, 1)?;
        let Action::Node(id, depth) = action else {
            if let Action::Text(text) = action {
                out.push(text)?;
            }
            continue;
        };
        out.budget().observe_depth(depth)?;
        let value = usize::try_from(id.0)
            .ok()
            .and_then(|i| package.bindings.get(i))
            .ok_or(PackageError::InvalidBinding)?;
        match value {
            Binding::None => out.push("[\"None\"]")?,
            Binding::Visit(v) | Binding::Import(v) | Binding::Propagate(v) => {
                out.push(match value {
                    Binding::Visit(_) => "[\"Visit\",",
                    Binding::Import(_) => "[\"Import\",",
                    _ => "[\"Propagate\",",
                })?;
                out.quoted(v)?;
                out.push("]")?;
            }
            Binding::Bind { namespace, name: n }
            | Binding::Reference { namespace, name: n }
            | Binding::Export { namespace, name: n } => {
                out.push(match value {
                    Binding::Bind { .. } => "[\"Bind\",",
                    Binding::Reference { .. } => "[\"Reference\",",
                    _ => "[\"Export\",",
                })?;
                out.quoted(namespace)?;
                out.push(",")?;
                name(out, n)?;
                out.push("]")?;
            }
            Binding::Sequential { declarations, body }
            | Binding::Recursive { declarations, body } => {
                out.push(if matches!(value, Binding::Sequential { .. }) {
                    "[\"Sequential\","
                } else {
                    "[\"Recursive\","
                })?;
                out.quoted(declarations)?;
                out.push(",")?;
                out.quoted(body)?;
                out.push("]")?;
            }
            Binding::Custom(v) => {
                out.push("[\"Custom\",")?;
                operation(out, v)?;
                out.push("]")?;
            }
            Binding::Group(v) | Binding::Scope(v) => {
                out.push(if matches!(value, Binding::Group(_)) {
                    "[\"Group\",["
                } else {
                    "[\"Scope\",["
                })?;
                push(&mut pending, Action::Text("]]"), out)?;
                let next = depth
                    .checked_add(1)
                    .ok_or(nepl3_core::budget::StopReason::DepthLimit)?;
                for (i, child) in v.iter().enumerate().rev() {
                    push(&mut pending, Action::Node(*child, next), out)?;
                    if i != 0 {
                        push(&mut pending, Action::Text(","), out)?;
                    }
                }
            }
        }
    }
    Ok(())
}

impl CheckedLanguagePackage<'_> {
    /// Canonical semantic metadata. The checked source/provenance tables remain separate.
    pub fn semantic_json(&self, budget: &mut Budget) -> Result<Vec<u8>, PackageError> {
        budget.charge(Resource::Work, 1)?;
        let p = self.package;
        let normalized = reader::normal_form(&p.reader, budget)?;
        let mut out = CanonicalWriter::new(budget);
        out.push("{\"categories\":[")?;
        for (i, c) in sorted(&p.categories, |c| (&c.name, ""), out.budget())?
            .iter()
            .enumerate()
        {
            comma(&mut out, i)?;
            out.push("[")?;
            out.quoted(&c.name)?;
            out.push(",")?;
            out.quoted(&c.mode)?;
            out.push("]")?;
        }
        out.push("],\"extensions\":[")?;
        for (i, e) in sorted(&p.extensions, |e| (&e.alias, ""), out.budget())?
            .iter()
            .enumerate()
        {
            comma(&mut out, i)?;
            out.push("[")?;
            out.quoted(&e.alias)?;
            out.push(",")?;
            out.quoted(&e.provider)?;
            out.push(",")?;
            out.quoted(&e.signature)?;
            out.push(",")?;
            operation(&mut out, &e.operation)?;
            out.push(",")?;
            out.ty(&e.input, 0)?;
            out.push(",")?;
            out.ty(&e.output, 0)?;
            out.push(if e.pure { ",true]" } else { ",false]" })?;
        }
        out.push("],\"forms\":[")?;
        for (i, f) in sorted(&p.forms, |f| (&f.category, &f.spelling), out.budget())?
            .iter()
            .enumerate()
        {
            comma(&mut out, i)?;
            out.push("[")?;
            out.quoted(&f.category)?;
            out.push(",")?;
            out.quoted(&f.spelling)?;
            out.push(",")?;
            kind(&mut out, &f.kind)?;
            out.push(",[")?;
            for (j, field) in f.fields.iter().enumerate() {
                comma(&mut out, j)?;
                out.push("[")?;
                out.quoted(&field.name)?;
                out.push(",")?;
                read(&mut out, p, field.read)?;
                out.push("]")?;
            }
            out.push("],")?;
            binding(&mut out, p, f.binding)?;
            out.push(",")?;
            styles(&mut out, &f.styles)?;
            out.push("]")?;
        }
        out.push("],\"leaves\":[")?;
        // Each category/token-kind pair is unique; kind names are canonical IDs.
        let mut leaves = sorted(&p.leaves, |l| (&l.category, ""), out.budget())?;
        for i in 1..leaves.len() {
            let mut j = i;
            while j > 0 {
                out.budget().charge(
                    Resource::Work,
                    leaves[j - 1].category.len().min(leaves[j].category.len()) as u64 + 1,
                )?;
                if (&leaves[j - 1].category, leaves[j - 1].token_kind.local_kind)
                    <= (&leaves[j].category, leaves[j].token_kind.local_kind)
                {
                    break;
                }
                leaves.swap(j - 1, j);
                j -= 1;
            }
        }
        for (i, l) in leaves.iter().enumerate() {
            comma(&mut out, i)?;
            out.push("[")?;
            out.quoted(&l.category)?;
            out.push(",")?;
            kind(&mut out, &l.kind)?;
            out.push(",")?;
            kind(&mut out, &l.token_kind)?;
            out.push(",")?;
            out.ty(&l.payload, 0)?;
            out.push(",")?;
            binding(&mut out, p, l.binding)?;
            out.push(",")?;
            styles(&mut out, &l.styles)?;
            out.push("]")?;
        }
        out.push("],\"modes\":[")?;
        for (i, m) in sorted(&p.modes, |m| (&m.name, ""), out.budget())?
            .iter()
            .enumerate()
        {
            comma(&mut out, i)?;
            out.push("[")?;
            out.quoted(&m.name)?;
            out.push(",[")?;
            for (j, s) in m.skip.iter().enumerate() {
                comma(&mut out, j)?;
                token_reader(&mut out, &s.reader)?;
            }
            out.push("],[")?;
            for (j, t) in m.take.iter().enumerate() {
                comma(&mut out, j)?;
                out.push("[")?;
                token_reader(&mut out, &t.reader)?;
                out.push(",")?;
                kind(&mut out, &t.kind)?;
                out.push("]")?;
            }
            out.push("]]")?;
        }
        out.push("],\"namespaces\":[")?;
        for (i, n) in sorted(&p.namespaces, |n| (&n.name, ""), out.budget())?
            .iter()
            .enumerate()
        {
            comma(&mut out, i)?;
            out.push("[")?;
            out.quoted(&n.name)?;
            out.push(",")?;
            out.quoted(match n.policy {
                NamespacePolicy::Lexical => "Lexical",
                NamespacePolicy::Global => "Global",
                NamespacePolicy::Open => "Open",
            })?;
            out.push("]")?;
        }
        out.push("],\"payloadSchemas\":[")?;
        let mut schemas = sorted(&p.payload_schemas, |s| (&s.package, ""), out.budget())?;
        for i in 1..schemas.len() {
            let mut j = i;
            while j > 0 {
                out.budget().charge(
                    Resource::Work,
                    schemas[j - 1].package.len().min(schemas[j].package.len()) as u64 + 33,
                )?;
                if schemas[j - 1] <= schemas[j] {
                    break;
                }
                schemas.swap(j - 1, j);
                j -= 1;
            }
        }
        for (i, s) in schemas.iter().enumerate() {
            comma(&mut out, i)?;
            schema(&mut out, s)?;
        }
        out.push("],\"reader\":")?;
        out.push(core::str::from_utf8(&normalized).map_err(|_| PackageError::KindShape)?)?;
        out.push(",\"recovery\":[")?;
        let policy = |v| match v {
            crate::recovery::UnexpectedPolicy::ConsumeToken => "ConsumeToken",
            crate::recovery::UnexpectedPolicy::PreserveRemainder => "PreserveRemainder",
        };
        out.quoted(policy(p.recovery.default_unexpected))?;
        out.push(",[")?;
        for (i, r) in sorted(&p.recovery.rules, |r| (&r.category, ""), out.budget())?
            .iter()
            .enumerate()
        {
            comma(&mut out, i)?;
            out.push("[")?;
            out.quoted(&r.category)?;
            out.push(",")?;
            out.quoted(policy(r.unexpected))?;
            out.push(",[")?;
            for (j, s) in r.synchronization.iter().enumerate() {
                comma(&mut out, j)?;
                out.push("[")?;
                out.quoted(&s.ancestor_category)?;
                out.push(",")?;
                kind(&mut out, &s.kind)?;
                out.push(",")?;
                if let Some(spelling) = &s.spelling {
                    out.quoted(spelling)?;
                } else {
                    out.push("null")?;
                }
                out.push("]")?;
            }
            out.push("]]")?;
        }
        out.push("]]")?;
        out.push(",\"root\":")?;
        out.quoted(&p.root)?;
        out.push(",\"schema\":")?;
        schema(&mut out, &p.schema)?;
        out.push("}")?;
        Ok(out.finish())
    }
    pub fn semantic_identity(&self, budget: &mut Budget) -> Result<PackageIdentity, PackageError> {
        let bytes = self.semantic_json(budget)?;
        budget.charge(Resource::Work, bytes.len() as u64)?;
        let digest = Digest::domain(b"NEPL3-PACKAGE-1\0", &bytes);
        budget.charge(
            Resource::AllocationUnits,
            self.package.schema.package.len() as u64,
        )?;
        Ok(PackageIdentity {
            schema: self.package.schema.clone(),
            semantic_digest: digest,
        })
    }
}
