use super::*;
use crate::model::{DocKind, DocNode, DocRoot, InlineRef, SentenceRef};
use alloc::{string::String, vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    origin::OriginId,
    schema::SchemaRegistry,
    source::SourceAdmission,
    value::{KindRef, SchemaRef},
    view::{ViewBundle, ViewElement, ViewField, ViewRef},
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum FrameKind {
    Sentence,
    Ruby,
    Anno,
}
struct Frame {
    kind: FrameKind,
    start: usize,
    items: Vec<InlineRef>,
    parts: Vec<InlineRef>,
    text: String,
    text_start: usize,
    text_end: usize,
    spelling: Vec<ViewRef>,
}
impl Frame {
    fn new(kind: FrameKind, start: usize) -> Self {
        Self {
            kind,
            start,
            items: Vec::new(),
            parts: Vec::new(),
            text: String::new(),
            text_start: start,
            text_end: start,
            spelling: Vec::new(),
        }
    }
}
struct Parser<'a> {
    source: &'a SourceSnapshot,
    schema: SchemaRef,
    kinds: [u64; 6],
    nodes: Vec<DocNode>,
    origins: Vec<Origin>,
    views: Vec<ViewElement>,
    node_views: Vec<ViewRef>,
}

/// Recognize one literal in the explicit UTF-8 window. Nonfinal incomplete input
/// returns NeedMore; callers restart with a new admitted snapshot explicitly.
pub fn read<'a>(
    source: &'a SourceSnapshot,
    start: u64,
    limit: u64,
    final_input: bool,
    registry: &SchemaRegistry,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<SentenceScan<'a>, SentenceError> {
    budget.poll()?;
    admission.admit_existing(source, budget)?;
    let input = source.slice_range(start, limit)?;
    budget.charge(Resource::Work, 1)?;
    if !input.starts_with('"') {
        return Ok(SentenceScan {
            source,
            outcome: if input.is_empty() && !final_input {
                SentenceOutcome::NeedMore
            } else {
                SentenceOutcome::NoMatch
            },
        });
    }
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    let schema = registry
        .selected("nepl3.doc", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    budget.charge(Resource::AllocationUnits, 64 + schema.package.len() as u64)?;
    let mut kinds = [0u64; 6];
    let descriptor = registry
        .descriptor(schema)
        .ok_or(SchemaError::UnknownSchema)?;
    for (slot, name) in kinds.iter_mut().zip([
        "View:Sentence",
        "View:TextRun",
        "View:Escape",
        "View:Ruby",
        "View:Anno",
        "View:Delimiter",
    ]) {
        let mut found = None;
        for (index, ty) in descriptor.types.iter().enumerate() {
            budget.charge(
                Resource::Work,
                (ty.name.len() as u64)
                    .saturating_add(name.len() as u64)
                    .saturating_add(1),
            )?;
            if ty.name == name {
                found = Some(index as u64);
                break;
            }
        }
        *slot = found.ok_or(SchemaError::UnknownType)?;
    }
    let mut p = Parser {
        source,
        schema: schema.clone(),
        kinds,
        nodes: Vec::new(),
        origins: Vec::new(),
        views: Vec::new(),
        node_views: Vec::new(),
    };
    let outcome = p.run(start as usize, limit as usize, final_input, budget)?;
    Ok(SentenceScan { source, outcome })
}
impl Parser<'_> {
    fn failure(
        &self,
        code: SentenceCode,
        at: usize,
        end: usize,
        opening: Option<usize>,
        b: &mut Budget,
    ) -> Result<SentenceOutcome, SentenceError> {
        let primary = self.source.span_with_budget(at as u64, end as u64, b)?;
        let opening = opening
            .map(|i| self.source.span_with_budget(i as u64, (i + 1) as u64, b))
            .transpose()?;
        Ok(SentenceOutcome::Failed(SentenceFailure {
            code,
            primary,
            opening,
        }))
    }
    fn view(
        &mut self,
        kind: usize,
        start: usize,
        end: usize,
        fields: Vec<ViewField>,
        b: &mut Budget,
    ) -> Result<ViewRef, SentenceError> {
        b.charge(Resource::Nodes, 1)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<ViewElement>() as u64 + self.schema.package.len() as u64,
        )?;
        let span = self.source.span_with_budget(start as u64, end as u64, b)?;
        let r = ViewRef(self.views.len() as u64);
        self.views.push(ViewElement {
            kind: KindRef {
                schema: self.schema.clone(),
                local_kind: self.kinds[kind],
            },
            span,
            fields,
            roles: vec![],
            relations: vec![],
        });
        Ok(r)
    }
    fn fields(
        &self,
        name: &str,
        children: &[InlineRef],
        b: &mut Budget,
    ) -> Result<Vec<ViewField>, SentenceError> {
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<ViewField>() as u64
                + name.len() as u64
                + (children.len() as u64) * 8,
        )?;
        let mut refs = Vec::with_capacity(children.len());
        for child in children {
            b.charge(Resource::Work, 1)?;
            refs.push(
                *self
                    .node_views
                    .get(child.0 as usize)
                    .ok_or(crate::check::ShapeError::Reference(child.0))?,
            );
        }
        Ok(vec![ViewField {
            name: String::from(name),
            children: refs,
        }])
    }
    fn node(
        &mut self,
        kind: DocKind,
        view_kind: usize,
        start: usize,
        end: usize,
        fields: Vec<ViewField>,
        b: &mut Budget,
    ) -> Result<InlineRef, SentenceError> {
        b.charge(Resource::Nodes, 1)?;
        b.charge(
            Resource::AllocationUnits,
            (core::mem::size_of::<DocNode>() + core::mem::size_of::<Origin>() + 8) as u64,
        )?;
        let span = self.source.span_with_budget(start as u64, end as u64, b)?;
        let origin = OriginId(self.origins.len() as u64);
        let origin_span = self.source.span_with_budget(start as u64, end as u64, b)?;
        self.origins.push(Origin::Direct(origin_span));
        let r = InlineRef(self.nodes.len() as u64);
        self.nodes.push(DocNode {
            locations: Vec::new(),
            kind,
            span: Some(span),
            origin: Some(origin),
        });
        let vr = self.view(view_kind, start, end, fields, b)?;
        self.node_views.push(vr);
        Ok(r)
    }
    fn flush(&mut self, f: &mut Frame, b: &mut Budget) -> Result<(), SentenceError> {
        if f.text.is_empty() {
            return Ok(());
        }
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<ViewField>() as u64 + 8 + 8,
        )?;
        let fields = vec![ViewField {
            name: String::from("spelling"),
            children: core::mem::take(&mut f.spelling),
        }];
        let node = self.node(
            DocKind::Text {
                text: core::mem::take(&mut f.text),
            },
            1,
            f.text_start,
            f.text_end,
            fields,
            b,
        )?;
        f.items.push(node);
        Ok(())
    }
    fn part(
        &mut self,
        f: &mut Frame,
        end: usize,
        b: &mut Budget,
    ) -> Result<Option<InlineRef>, SentenceError> {
        self.flush(f, b)?;
        if f.items.is_empty() {
            return Ok(None);
        }
        if f.items.len() == 1 {
            return Ok(f.items.pop());
        }
        let start = self
            .nodes
            .get(f.items[0].0 as usize)
            .and_then(|n| n.span.as_ref())
            .map(|s| s.start() as usize)
            .ok_or(crate::check::ShapeError::Reference(f.items[0].0))?;
        let fields = self.fields("items", &f.items, b)?;
        let items = core::mem::take(&mut f.items);
        Ok(Some(self.node(
            DocKind::Concat { inlines: items },
            0,
            start,
            end,
            fields,
            b,
        )?))
    }
    fn run(
        &mut self,
        start: usize,
        limit: usize,
        final_input: bool,
        b: &mut Budget,
    ) -> Result<SentenceOutcome, SentenceError> {
        let base = b.current_depth();
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Frame>() as u64,
        )?;
        let mut stack = vec![Frame::new(FrameKind::Sentence, start)];
        let mut cursor = start + 1;
        loop {
            b.with_depth_at_least::<_, SentenceError>(
                base.saturating_add(stack.len() as u64),
                |b| {
                    b.charge(Resource::Work, 1)?;
                    Ok(())
                },
            )?;
            let Some(mut f) = stack.pop() else {
                return Err(crate::check::ShapeError::Reference(0).into());
            };
            let ch = self
                .source
                .text()
                .get(cursor..limit)
                .and_then(|s| s.chars().next());
            let Some(ch) = ch else {
                if !final_input {
                    return Ok(SentenceOutcome::NeedMore);
                }
                return self.failure(
                    if f.kind == FrameKind::Sentence {
                        SentenceCode::UnterminatedLiteral
                    } else {
                        SentenceCode::UnclosedAnnotation
                    },
                    cursor,
                    cursor,
                    (f.kind != FrameKind::Sentence).then_some(f.start),
                    b,
                );
            };
            if ch == '"' || matches!(ch, '\r' | '\n') {
                if f.kind != FrameKind::Sentence {
                    return self.failure(
                        SentenceCode::UnclosedAnnotation,
                        cursor,
                        cursor + ch.len_utf8(),
                        Some(f.start),
                        b,
                    );
                }
                if ch != '"' {
                    return self.failure(
                        SentenceCode::DirectLineBreak,
                        cursor,
                        cursor + 1,
                        None,
                        b,
                    );
                }
                self.flush(&mut f, b)?;
                let fields = self.fields("items", &f.items, b)?;
                let root = self.node(
                    DocKind::Sentence { inlines: f.items },
                    0,
                    start,
                    cursor + 1,
                    fields,
                    b,
                )?;
                let value = DocValue {
                    root: DocRoot::Sentence(SentenceRef(root.0)),
                    nodes: core::mem::take(&mut self.nodes),
                    embeds: vec![],
                };
                let value = crate::normalize::value(value, &mut self.origins, b)?;
                let head = self
                    .source
                    .span_with_budget(start as u64, (cursor + 1) as u64, b)?;
                let owner = self
                    .source
                    .span_with_budget(start as u64, (cursor + 1) as u64, b)?;
                b.charge(Resource::AllocationUnits, 8)?;
                let root_view = *self
                    .node_views
                    .get(root.0 as usize)
                    .ok_or(crate::check::ShapeError::Reference(root.0))?;
                return Ok(SentenceOutcome::Matched(SentenceLiteral {
                    head,
                    value,
                    origins: core::mem::take(&mut self.origins),
                    view: DocView {
                        head: owner,
                        view: ViewBundle {
                            elements: core::mem::take(&mut self.views),
                            roots: vec![root_view],
                        },
                    },
                }));
            }
            if matches!(ch, '[' | '{') {
                self.flush(&mut f, b)?;
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<Frame>() as u64,
                )?;
                stack.push(f);
                stack.push(Frame::new(
                    if ch == '[' {
                        FrameKind::Ruby
                    } else {
                        FrameKind::Anno
                    },
                    cursor,
                ));
                cursor += 1;
                continue;
            }
            if ch == '/' && f.kind != FrameKind::Sentence {
                if f.kind == FrameKind::Ruby && !f.parts.is_empty() {
                    return self.failure(
                        SentenceCode::SeparatorCount,
                        cursor,
                        cursor + 1,
                        Some(f.start),
                        b,
                    );
                }
                let Some(part) = self.part(&mut f, cursor, b)? else {
                    return self.failure(
                        SentenceCode::EmptyAnnotationPart,
                        cursor,
                        cursor,
                        Some(f.start),
                        b,
                    );
                };
                b.charge(Resource::AllocationUnits, 8)?;
                f.parts.push(part);
                cursor += 1;
                stack.push(f);
                continue;
            }
            if matches!(ch, ']' | '}') {
                if !matches!(
                    (f.kind, ch),
                    (FrameKind::Ruby, ']') | (FrameKind::Anno, '}')
                ) {
                    return self.failure(
                        SentenceCode::UnexpectedDelimiter,
                        cursor,
                        cursor + 1,
                        None,
                        b,
                    );
                }
                let Some(part) = self.part(&mut f, cursor, b)? else {
                    return self.failure(
                        SentenceCode::EmptyAnnotationPart,
                        cursor,
                        cursor,
                        Some(f.start),
                        b,
                    );
                };
                if f.parts.is_empty() {
                    return self.failure(
                        SentenceCode::SeparatorCount,
                        cursor,
                        cursor + 1,
                        Some(f.start),
                        b,
                    );
                }
                b.charge(Resource::AllocationUnits, 8)?;
                f.parts.push(part);
                let base = f.parts[0];
                let mut fields = self.fields("base", &[base], b)?;
                fields.extend(self.fields(
                    if f.kind == FrameKind::Ruby {
                        "reading"
                    } else {
                        "notes"
                    },
                    &f.parts[1..],
                    b,
                )?);
                let kind = if f.kind == FrameKind::Ruby {
                    DocKind::Ruby {
                        base,
                        reading: f.parts[1],
                    }
                } else {
                    DocKind::Anno {
                        base,
                        notes: f.parts.into_iter().skip(1).collect(),
                    }
                };
                let node = self.node(
                    kind,
                    if f.kind == FrameKind::Ruby { 3 } else { 4 },
                    f.start,
                    cursor + 1,
                    fields,
                    b,
                )?;
                let parent = stack
                    .last_mut()
                    .ok_or(crate::check::ShapeError::Reference(node.0))?;
                b.charge(Resource::AllocationUnits, 8)?;
                parent.items.push(node);
                cursor += 1;
                continue;
            }
            let at = cursor;
            let (decoded, end, escaped) = if ch == '\\' {
                match self.escape(cursor, limit, b)? {
                    Escape::Value(ch, end) => (ch, end, true),
                    Escape::Incomplete => {
                        if !final_input {
                            return Ok(SentenceOutcome::NeedMore);
                        }
                        return self.failure(SentenceCode::InvalidEscape, cursor, limit, None, b);
                    }
                    Escape::Invalid(code, end) => return self.failure(code, cursor, end, None, b),
                }
            } else {
                (ch, cursor + ch.len_utf8(), false)
            };
            b.charge(Resource::AllocationUnits, decoded.len_utf8() as u64 + 8)?;
            if f.text.is_empty() {
                f.text_start = at;
            }
            f.text.push(decoded);
            f.text_end = end;
            let spelling = self.view(if escaped { 2 } else { 1 }, at, end, vec![], b)?;
            f.spelling.push(spelling);
            cursor = end;
            stack.push(f);
        }
    }
    fn escape(&self, start: usize, limit: usize, b: &mut Budget) -> Result<Escape, SentenceError> {
        let mut cursor = start + 1;
        b.charge(Resource::Work, 1)?;
        let Some(ch) = self
            .source
            .text()
            .get(cursor..limit)
            .and_then(|s| s.chars().next())
        else {
            return Ok(Escape::Incomplete);
        };
        cursor += ch.len_utf8();
        let value = match ch {
            '\\' | '"' | '[' | ']' | '{' | '}' | '/' => ch,
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'u' => {
                if cursor == limit {
                    return Ok(Escape::Incomplete);
                }
                if self.source.text().as_bytes().get(cursor) != Some(&b'{') {
                    return Ok(Escape::Invalid(SentenceCode::InvalidEscape, cursor));
                }
                cursor += 1;
                let mut number = 0u32;
                let mut digits = 0;
                loop {
                    b.charge(Resource::Work, 1)?;
                    let Some(ch) = self
                        .source
                        .text()
                        .get(cursor..limit)
                        .and_then(|s| s.chars().next())
                    else {
                        return Ok(Escape::Incomplete);
                    };
                    if ch == '}' {
                        cursor += 1;
                        break;
                    }
                    let Some(d) = ch.to_digit(16).filter(|_| ch.is_ascii_hexdigit()) else {
                        return Ok(Escape::Invalid(
                            SentenceCode::InvalidEscape,
                            cursor + ch.len_utf8(),
                        ));
                    };
                    if digits == 6 {
                        return Ok(Escape::Invalid(SentenceCode::InvalidScalar, cursor + 1));
                    }
                    number = number * 16 + d;
                    digits += 1;
                    cursor += 1;
                }
                let Some(ch) = char::from_u32(number).filter(|_| digits != 0) else {
                    return Ok(Escape::Invalid(SentenceCode::InvalidScalar, cursor));
                };
                ch
            }
            _ => return Ok(Escape::Invalid(SentenceCode::InvalidEscape, cursor)),
        };
        Ok(Escape::Value(value, cursor))
    }
}
enum Escape {
    Value(char, usize),
    Incomplete,
    Invalid(SentenceCode, usize),
}
