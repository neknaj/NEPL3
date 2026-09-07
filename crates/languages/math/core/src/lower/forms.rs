use super::*;
use nepl3_core::value::NdfValue;
impl<'a> Adapter<'a, '_> {
    pub(super) fn convert_at_depth(
        &mut self,
        id: NodeRef,
        node: &SyntaxNode,
        depth: u64,
        admission: &mut SourceAdmission,
    ) -> Result<(), LowerError> {
        let Self {
            checked,
            registry,
            mapping,
            nodes,
            embeds,
            b,
        } = self;
        b.with_depth_at_least(depth, |b| {
            let mut adapter = Adapter {
                checked,
                registry,
                mapping: core::mem::take(mapping),
                nodes: core::mem::take(nodes),
                embeds: core::mem::take(embeds),
                b,
            };
            let result = adapter.convert(id, node, admission);
            *mapping = adapter.mapping;
            *nodes = adapter.nodes;
            *embeds = adapter.embeds;
            result
        })
    }
    fn token(
        &self,
        id: NodeRef,
        n: &SyntaxNode,
    ) -> Result<&'a nepl3_core::view::Token, LowerError> {
        n.token
            .and_then(|t| self.checked.bundle().tokens.get(t.0 as usize))
            .ok_or(LowerError::Operand { node: id, field: 0 })
    }
    fn convert(
        &mut self,
        id: NodeRef,
        n: &SyntaxNode,
        admission: &mut SourceAdmission,
    ) -> Result<(), LowerError> {
        let kind = match (n.kind.as_str(), n.fields.len()) {
            ("Builtin:Text" | "Builtin:Name", 0) => return Ok(()),
            (name, 0 | 2) if name.starts_with("List:") => return Ok(()),
            ("Leaf:Number", 0) => {
                let token = self.token(id, n)?;
                let NdfValue::Rational(value) = &token.payload else {
                    return Err(LowerError::Operand { node: id, field: 0 });
                };
                let value = crate::number::clone_with_budget(value, self.b)?;
                let spelling = Some(super::span(&self.token(id, n)?.head, self.b)?);
                MathKind::Number { value, spelling }
            }
            ("Leaf:SymbolName", 0) => {
                let token = self.token(id, n)?;
                let NdfValue::Text(text) = &token.payload else {
                    return Err(LowerError::Operand { node: id, field: 0 });
                };
                self.b.charge(Resource::Work, text.len() as u64)?;
                self.b
                    .charge(Resource::AllocationUnits, text.len() as u64)?;
                MathKind::Symbol { name: text.clone() }
            }
            ("Form:Add", 2) => MathKind::Add {
                left: ExprRef(self.node(id, n, 0, Category::Expr)?),
                right: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Sub", 2) => MathKind::Sub {
                left: ExprRef(self.node(id, n, 0, Category::Expr)?),
                right: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Mul", 2) => MathKind::Mul {
                left: ExprRef(self.node(id, n, 0, Category::Expr)?),
                right: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Frac", 2) => MathKind::Frac {
                left: ExprRef(self.node(id, n, 0, Category::Expr)?),
                right: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Pow", 2) => MathKind::Pow {
                left: ExprRef(self.node(id, n, 0, Category::Expr)?),
                right: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Equal", 2) => MathKind::Equal {
                left: ExprRef(self.node(id, n, 0, Category::Expr)?),
                right: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Lt", 2) => MathKind::Lt {
                left: ExprRef(self.node(id, n, 0, Category::Expr)?),
                right: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Le", 2) => MathKind::Le {
                left: ExprRef(self.node(id, n, 0, Category::Expr)?),
                right: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Subscript", 2) => MathKind::Subscript {
                left: ExprRef(self.node(id, n, 0, Category::Expr)?),
                right: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Superscript", 2) => MathKind::Superscript {
                left: ExprRef(self.node(id, n, 0, Category::Expr)?),
                right: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Neg", 1) => MathKind::Neg {
                value: ExprRef(self.node(id, n, 0, Category::Expr)?),
            },
            ("Form:Sqrt", 1) => MathKind::Sqrt {
                value: ExprRef(self.node(id, n, 0, Category::Expr)?),
            },
            ("Form:Transpose", 1) => MathKind::Transpose {
                value: ExprRef(self.node(id, n, 0, Category::Expr)?),
            },
            ("Form:Det", 1) => MathKind::Det {
                value: ExprRef(self.node(id, n, 0, Category::Expr)?),
            },
            ("Form:Root", 2) => MathKind::Root {
                degree: ExprRef(self.node(id, n, 0, Category::Expr)?),
                radicand: ExprRef(self.node(id, n, 1, Category::Expr)?),
            },
            ("Form:Scripts", 3) => MathKind::Scripts {
                base: ExprRef(self.node(id, n, 0, Category::Expr)?),
                sub: ExprRef(self.node(id, n, 1, Category::Expr)?),
                sup: ExprRef(self.node(id, n, 2, Category::Expr)?),
            },
            ("Form:Fence", 3) => MathKind::Fence {
                open: self.text(id, n, 0, "Builtin:Text")?,
                close: self.text(id, n, 1, "Builtin:Text")?,
                value: ExprRef(self.node(id, n, 2, Category::Expr)?),
            },
            ("Form:Sequence", 1) => MathKind::Sequence {
                values: self
                    .list(id, n, 0, Category::Expr)?
                    .into_iter()
                    .map(ExprRef)
                    .collect(),
            },
            ("Form:Symbol", 1) => MathKind::Symbol {
                name: self.text(id, n, 0, "Builtin:Text")?,
            },
            ("Form:Text", 1) => MathKind::Text {
                text: self.text(id, n, 0, "Builtin:Text")?,
            },
            ("Form:Vector", 1) => MathKind::Vector {
                values: self
                    .list(id, n, 0, Category::Expr)?
                    .into_iter()
                    .map(ExprRef)
                    .collect(),
            },
            ("Form:Matrix", 1) => MathKind::Matrix {
                rows: self
                    .list(id, n, 0, Category::Row)?
                    .into_iter()
                    .map(RowRef)
                    .collect(),
            },
            ("Form:Let", 3) => MathKind::Let {
                name: self.text(id, n, 0, "Builtin:Name")?,
                init: ExprRef(self.node(id, n, 1, Category::Expr)?),
                body: ExprRef(self.node(id, n, 2, Category::Expr)?),
            },
            ("Form:Sum", 4) => MathKind::Sum {
                index: self.text(id, n, 0, "Builtin:Name")?,
                lower: ExprRef(self.node(id, n, 1, Category::Expr)?),
                upper: ExprRef(self.node(id, n, 2, Category::Expr)?),
                body: ExprRef(self.node(id, n, 3, Category::Expr)?),
            },
            ("Form:Integral", 4) => MathKind::Integral {
                index: self.text(id, n, 0, "Builtin:Name")?,
                lower: ExprRef(self.node(id, n, 1, Category::Expr)?),
                upper: ExprRef(self.node(id, n, 2, Category::Expr)?),
                body: ExprRef(self.node(id, n, 3, Category::Expr)?),
            },
            ("Form:Call", 2) => MathKind::Call {
                function: ExprRef(self.node(id, n, 0, Category::Expr)?),
                arguments: self
                    .list(id, n, 1, Category::Expr)?
                    .into_iter()
                    .map(ExprRef)
                    .collect(),
            },
            ("Form:Label", 2) => MathKind::Label {
                value: ExprRef(self.node(id, n, 0, Category::Expr)?),
                annotation: DocGuestRef(self.node(id, n, 1, Category::DocGuest)?),
            },
            ("Form:Row", 1) => MathKind::Row {
                values: self
                    .list(id, n, 0, Category::Expr)?
                    .into_iter()
                    .map(ExprRef)
                    .collect(),
            },
            ("Form:DocGuest", 1) => {
                let Some(FieldValue::Foreign(foreign)) = n.fields.first() else {
                    return Err(LowerError::Operand { node: id, field: 0 });
                };
                let closure = ForeignClosure::capture(
                    foreign,
                    self.checked,
                    self.registry,
                    self.b,
                    admission,
                )?;
                let syntax = EmbedRef(self.embeds.len() as u64);
                push(&mut self.embeds, closure, self.b)?;
                MathKind::DocGuest { syntax }
            }
            _ => return Err(LowerError::Unsupported { node: id }),
        };
        self.b.charge(Resource::Nodes, 1)?;
        let index = self.nodes.len() as u64;
        let span = n
            .cover
            .as_ref()
            .map(|s| super::span(s, self.b))
            .transpose()?;
        let field = match kind {
            MathKind::Symbol { .. } => Some(MathField::SymbolName),
            MathKind::Let { .. } => Some(MathField::LetName),
            MathKind::Sum { .. } => Some(MathField::SumIndex),
            MathKind::Integral { .. } => Some(MathField::IntegralIndex),
            _ => None,
        };
        let mut locations = Vec::new();
        if let Some(field) = field {
            let child = if n.kind == "Leaf:SymbolName" {
                id
            } else {
                self.child(id, n, 0)?
            };
            let child = &self.checked.bundle().nodes[child.0 as usize];
            let token = child
                .token
                .and_then(|t| self.checked.bundle().tokens.get(t.0 as usize))
                .ok_or(LowerError::Operand { node: id, field: 0 })?;
            let location = MathFieldLocation {
                field,
                origin: Some(child.origin),
                span: Some(super::span(&token.head, self.b)?),
            };
            push(&mut locations, location, self.b)?;
        }
        push(
            &mut self.nodes,
            MathNode {
                kind,
                origin: Some(n.origin),
                span,
                locations,
            },
            self.b,
        )?;
        self.mapping[id.0 as usize] = Some(Mapped::Node(index));
        Ok(())
    }
}
