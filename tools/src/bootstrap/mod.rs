//! Host-only import of the bounded first-seed JSON. This does not parse Grammar
//! or certify bootstrap: production parser output is lowered by grammar-core.
pub mod catalog;
pub mod cli;
mod generated;
pub mod runtime;
#[cfg(test)]
mod tests;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::{SourceAdmission, SourceError, SourceId, SourceSnapshot, Span},
    value::{Integer, decimal::DecimalError},
};
use nepl3_grammar_core::model::*;
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Debug)]
pub enum SeedError {
    Stopped(StopReason),
    Json(String),
    Shape,
    Source(SourceError),
    Model(ModelError),
    Digest,
    Literal,
}
impl From<StopReason> for SeedError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<SourceError> for SeedError {
    fn from(v: SourceError) -> Self {
        Self::Source(v)
    }
}
impl From<ModelError> for SeedError {
    fn from(v: ModelError) -> Self {
        Self::Model(v)
    }
}
struct Adapter<'a> {
    source: &'a SourceSnapshot,
    nodes: Vec<Node>,
    budget: &'a mut Budget,
    completed: BTreeMap<usize, NodeId>,
}
impl Adapter<'_> {
    fn string<'a>(&self, v: &'a Value, key: &str) -> Result<&'a str, SeedError> {
        v.get(key).and_then(Value::as_str).ok_or(SeedError::Shape)
    }
    fn fields(&mut self, v: &Value, expected: &[&str]) -> Result<(), SeedError> {
        let fields = v
            .get("fields")
            .and_then(Value::as_object)
            .ok_or(SeedError::Shape)?;
        self.budget.charge(Resource::Work, fields.len() as u64)?;
        if fields.len() != expected.len() || expected.iter().any(|k| !fields.contains_key(*k)) {
            return Err(SeedError::Shape);
        }
        Ok(())
    }
    fn span_value(&mut self, v: &Value) -> Result<Span, SeedError> {
        let range = v
            .as_array()
            .filter(|v| v.len() == 2)
            .ok_or(SeedError::Shape)?;
        let start = range[0].as_u64().ok_or(SeedError::Shape)?;
        let end = range[1].as_u64().ok_or(SeedError::Shape)?;
        Ok(self.source.span_with_budget(start, end, self.budget)?)
    }
    fn span(&mut self, v: &Value) -> Result<Span, SeedError> {
        self.span_value(v.get("span").ok_or(SeedError::Shape)?)
    }
    fn head(&mut self, v: &Value, spelling: &str) -> Result<(), SeedError> {
        let head = self.span_value(v.get("head").ok_or(SeedError::Shape)?)?;
        let cover = self.span(v)?;
        self.budget.charge(Resource::Work, spelling.len() as u64)?;
        if !cover.contains(&head)
            || cover.start() != head.start()
            || self.source.slice(&head)? != spelling
        {
            return Err(SeedError::Shape);
        }
        Ok(())
    }
    fn field<'a>(&self, v: &'a Value, name: &str) -> Result<&'a Value, SeedError> {
        v.get("fields")
            .and_then(|v| v.get(name))
            .ok_or(SeedError::Shape)
    }
    // Addresses are temporary host lookup keys only. They never enter the typed
    // document, identity, wire format, or generated metadata. JSON stays borrowed
    // and immutable for the entire postorder traversal.
    fn key(value: &Value) -> usize {
        core::ptr::from_ref(value) as usize
    }
    fn parse(&mut self, value: &Value, category: Category) -> Result<NodeId, SeedError> {
        let mut pending = Vec::new();
        self.schedule(&mut pending, value, false, 1)?;
        while let Some((value, exit, depth)) = pending.pop() {
            self.budget.observe_depth(depth)?;
            self.budget.charge(Resource::Work, 1)?;
            if exit {
                let kind = self.constructor(value)?;
                let span = self.span(value)?;
                self.budget.charge(Resource::Nodes, 1)?;
                self.budget.charge(
                    Resource::AllocationUnits,
                    (core::mem::size_of::<Node>() + core::mem::size_of::<(usize, NodeId)>()) as u64,
                )?;
                let id = NodeId(self.nodes.len() as u64);
                self.nodes.push(Node { kind, span });
                if self.completed.insert(Self::key(value), id).is_some() {
                    return Err(SeedError::Shape);
                }
            } else {
                self.schedule(&mut pending, value, true, depth)?;
                let fields = value
                    .get("fields")
                    .and_then(Value::as_object)
                    .ok_or(SeedError::Shape)?;
                let next_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| self.budget.stop(StopReason::DepthLimit))?;
                for field in fields.values().rev() {
                    self.budget.charge(Resource::Work, 1)?;
                    if field.get("literal").is_some() {
                        continue;
                    }
                    if let Some(items) = field.get("list").and_then(Value::as_array) {
                        for item in items.iter().rev() {
                            self.schedule(&mut pending, item, false, next_depth)?;
                        }
                    } else {
                        self.schedule(&mut pending, field, false, next_depth)?;
                    }
                }
            }
        }
        self.lookup(value, category)
    }
    fn schedule<'v>(
        &mut self,
        pending: &mut Vec<(&'v Value, bool, u64)>,
        value: &'v Value,
        exit: bool,
        depth: u64,
    ) -> Result<(), SeedError> {
        self.budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(&Value, bool, u64)>() as u64,
        )?;
        pending.push((value, exit, depth));
        Ok(())
    }
    fn lookup(&mut self, value: &Value, category: Category) -> Result<NodeId, SeedError> {
        self.budget.charge(
            Resource::Work,
            self.completed.len().checked_ilog2().unwrap_or(0) as u64 + 1,
        )?;
        let id = *self
            .completed
            .get(&Self::key(value))
            .ok_or(SeedError::Shape)?;
        let node = self
            .nodes
            .get(usize::try_from(id.0).map_err(|_| SeedError::Shape)?)
            .ok_or(SeedError::Shape)?;
        if node.kind.category() != category {
            return Err(SeedError::Shape);
        }
        Ok(id)
    }
    fn node(&mut self, v: &Value, name: &str, category: Category) -> Result<NodeId, SeedError> {
        self.lookup(self.field(v, name)?, category)
    }
    fn list(&mut self, v: &Value, name: &str, category: Category) -> Result<NodeList, SeedError> {
        let value = self.field(v, name)?;
        let values = value
            .get("list")
            .and_then(Value::as_array)
            .ok_or(SeedError::Shape)?;
        let heads = value
            .get("heads")
            .and_then(Value::as_array)
            .ok_or(SeedError::Shape)?;
        if heads.len() != values.len() + 1 {
            return Err(SeedError::Shape);
        }
        let span = self.span(value)?;
        let mut items = Vec::new();
        for (i, head) in heads.iter().enumerate() {
            let head = self.span_value(head)?;
            let raw = self.source.slice(&head)?;
            self.budget.charge(Resource::Work, raw.len() as u64)?;
            if !span.contains(&head)
                || raw != if i == values.len() { "nil" } else { "cons" }
                || i == 0 && head.start() != span.start()
                || i == values.len() && head.end() != span.end()
            {
                return Err(SeedError::Shape);
            }
            if let Some(value) = values.get(i) {
                let id = self.lookup(value, category)?;
                let child = &self.nodes[id.0 as usize].span;
                let next = heads
                    .get(i + 1)
                    .ok_or(SeedError::Shape)?
                    .get(0)
                    .and_then(Value::as_u64)
                    .ok_or(SeedError::Shape)?;
                if child.start() < head.end() || child.end() > next {
                    return Err(SeedError::Shape);
                }
                self.budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<NodeId>() as u64,
                )?;
                items.push(id);
            }
        }
        Ok(NodeList { items, span })
    }
    fn literal<'a>(
        &mut self,
        v: &'a Value,
        name: &str,
        kind: &str,
    ) -> Result<(&'a str, Span), SeedError> {
        let value = self.field(v, name)?;
        if self.string(value, "literal")? != kind {
            return Err(SeedError::Literal);
        }
        let span = self.span(value)?;
        let raw = self.string(value, "raw")?;
        self.budget.charge(Resource::Work, raw.len() as u64)?;
        if self.source.slice(&span)? != raw {
            return Err(SeedError::Literal);
        }
        Ok((self.string(value, "value")?, span))
    }
    fn name(&mut self, v: &Value, name: &str) -> Result<NameLiteral, SeedError> {
        let (value, span) = self.literal(v, name, "Name")?;
        self.budget.charge(Resource::Work, value.len() as u64)?;
        let mut chars = value.chars();
        if !chars
            .next()
            .is_some_and(|v| v == '_' || v.is_ascii_alphabetic())
            || !chars.all(|v| v == '_' || v.is_ascii_alphanumeric())
            || self.source.slice(&span)? != value
        {
            return Err(SeedError::Literal);
        }
        self.budget
            .charge(Resource::AllocationUnits, value.len() as u64)?;
        Ok(Located {
            value: value.into(),
            span,
        })
    }
    fn text(&mut self, v: &Value, name: &str) -> Result<TextLiteral, SeedError> {
        let (value, span) = self.literal(v, name, "Text")?;
        let raw = self.source.slice(&span)?;
        let decoded = decode_text(raw, self.budget)?;
        if decoded != value {
            return Err(SeedError::Literal);
        }
        Ok(Located {
            value: decoded,
            span,
        })
    }
    fn nat(&mut self, v: &Value, name: &str) -> Result<NatLiteral, SeedError> {
        let (value, span) = self.literal(v, name, "Nat")?;
        if value.is_empty()
            || value.len() > 1 && value.starts_with('0')
            || !value.bytes().all(|v| v.is_ascii_digit())
            || self.source.slice(&span)? != value
        {
            return Err(SeedError::Literal);
        }
        let value = Integer::from_decimal_digits(value, self.budget).map_err(|v| match v {
            DecimalError::Stopped(r) => SeedError::Stopped(r),
            _ => SeedError::Literal,
        })?;
        Ok(Located { value, span })
    }
}
fn decode_text(raw: &str, budget: &mut Budget) -> Result<String, SeedError> {
    budget.charge(Resource::Work, raw.len() as u64)?;
    budget.charge(Resource::AllocationUnits, raw.len() as u64)?;
    let inner = raw
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .ok_or(SeedError::Literal)?;
    let mut chars = inner.chars();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c == '\r' || c == '\n' || c == '"' {
            return Err(SeedError::Literal);
        }
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next().ok_or(SeedError::Literal)? {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'u' => {
                if chars.next() != Some('{') {
                    return Err(SeedError::Literal);
                }
                let mut scalar = 0u32;
                let mut count = 0;
                loop {
                    let c = chars.next().ok_or(SeedError::Literal)?;
                    if c == '}' {
                        break;
                    }
                    let digit = c
                        .to_digit(16)
                        .filter(|_| c.is_ascii_hexdigit())
                        .ok_or(SeedError::Literal)?;
                    count += 1;
                    if count > 6 {
                        return Err(SeedError::Literal);
                    }
                    scalar = scalar * 16 + digit;
                }
                if count == 0 {
                    return Err(SeedError::Literal);
                }
                out.push(char::from_u32(scalar).ok_or(SeedError::Literal)?);
            }
            _ => return Err(SeedError::Literal),
        }
    }
    Ok(out)
}
/// Load the explicit bounded seed interchange (JSON nesting is limited by serde_json).
/// Source identity and every retained literal/constructor/list span are checked.
pub fn load(
    bytes: &[u8],
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Document, SeedError> {
    budget.charge(Resource::Work, bytes.len() as u64)?;
    budget.observe_depth(1)?;
    let json_storage = (bytes.len() as u64)
        .checked_mul(128)
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::AllocationUnits, json_storage)?;
    let text = core::str::from_utf8(bytes).map_err(|v| SeedError::Json(v.to_string()))?;
    crate::repository::json::validate(text).map_err(|v| SeedError::Json(v.to_string()))?;
    let value: Value = serde_json::from_str(text).map_err(|v| SeedError::Json(v.to_string()))?;
    if value.get("schema").and_then(Value::as_str) != Some("nepl3.grammar-seed-input/1") {
        return Err(SeedError::Shape);
    }
    let raw = value.get("source").ok_or(SeedError::Shape)?;
    let string = |key: &str| raw.get(key).and_then(Value::as_str).ok_or(SeedError::Shape);
    let source = admission.import(
        SourceId(string("sourceId")?.into()),
        raw.get("revision")
            .and_then(Value::as_u64)
            .ok_or(SeedError::Shape)?,
        string("uri")?.into(),
        string("text")?.as_bytes().to_vec(),
        budget,
    )?;
    let expected = string("digest")?;
    if expected.len() != 64 {
        return Err(SeedError::Digest);
    }
    let mut digest = [0; 32];
    for (i, chunk) in expected.as_bytes().chunks_exact(2).enumerate() {
        let text = core::str::from_utf8(chunk).map_err(|_| SeedError::Digest)?;
        digest[i] = u8::from_str_radix(text, 16).map_err(|_| SeedError::Digest)?;
    }
    if digest != source.identity().digest.0 {
        return Err(SeedError::Digest);
    }
    let mut adapter = Adapter {
        source: &source,
        nodes: Vec::new(),
        budget,
        completed: BTreeMap::new(),
    };
    let root = adapter.parse(value.get("root").ok_or(SeedError::Shape)?, Category::Root)?;
    let nodes = adapter.nodes;
    let document = Document {
        sources: vec![source],
        nodes,
        root,
    };
    document.validate(budget, admission)?;
    Ok(document)
}
