use super::*;
impl Adapter<'_, '_> {
    pub(super) fn convert_at_depth(
        &mut self,
        id: NodeRef,
        node: &SyntaxNode,
        depth: u64,
    ) -> Result<(), LowerError> {
        let Self {
            checked,
            registry,
            mapping,
            nodes,
            embeds,
            b,
            admission,
        } = self;
        b.with_depth_at_least(depth, |b| {
            let mut adapter = Adapter {
                checked,
                registry,
                mapping: core::mem::take(mapping),
                nodes: core::mem::take(nodes),
                embeds: core::mem::take(embeds),
                b,
                admission,
            };
            let result = adapter.convert(id, node);
            *mapping = adapter.mapping;
            *nodes = adapter.nodes;
            *embeds = adapter.embeds;
            result
        })
    }
    pub(super) fn convert(&mut self, id: NodeRef, n: &SyntaxNode) -> Result<(), LowerError> {
        use Category as C;
        let kind = match (n.kind.as_str(), n.fields.len()) {
            ("Builtin:Text" | "Builtin:Name" | "Builtin:Lang" | "Builtin:Nat", 0) => return Ok(()),
            (name, 0 | 2) if name.starts_with("List:") => return Ok(()),
            ("Form:MathGuest" | "Form:CircuitGuest" | "Form:GrammarGuest" | "Form:DocGuest", 1) => {
                if !matches!(n.fields.first(), Some(FieldValue::Foreign(_))) {
                    return Err(LowerError::Operand { node: id, field: 0 });
                }
                return Ok(());
            }
            ("Form:Article", 3) => DocKind::Article {
                language: self.text(id, n, 0, "Builtin:Lang")?,
                title: SentenceRef(self.node(id, n, 1, C::Sentence)?),
                body: BodyRef(self.node(id, n, 2, C::Body)?),
            },
            ("Form:Body", 1) => DocKind::Body {
                blocks: self
                    .list(id, n, 0, C::Block)?
                    .into_iter()
                    .map(BlockRef)
                    .collect(),
            },
            ("Form:Paragraph", 1) => DocKind::Paragraph {
                items: self
                    .list(id, n, 0, C::Flow)?
                    .into_iter()
                    .map(FlowRef)
                    .collect(),
            },
            ("Form:Section", 3) => DocKind::Section {
                id: self.text(id, n, 0, "Builtin:Name")?,
                title: SentenceRef(self.node(id, n, 1, C::Sentence)?),
                body: BodyRef(self.node(id, n, 2, C::Body)?),
            },
            ("Form:Sentence", 1) => DocKind::Sentence {
                inlines: self
                    .list(id, n, 0, C::Inline)?
                    .into_iter()
                    .map(InlineRef)
                    .collect(),
            },
            ("Form:Parallel", 1) => DocKind::Parallel {
                variants: self
                    .list(id, n, 0, C::Variant)?
                    .into_iter()
                    .map(VariantRef)
                    .collect(),
            },
            ("Form:Variant", 2) => DocKind::Variant {
                language: self.text(id, n, 0, "Builtin:Lang")?,
                sentence: SentenceRef(self.node(id, n, 1, C::Sentence)?),
            },
            ("Form:Text", 1) => DocKind::Text {
                text: self.text(id, n, 0, "Builtin:Text")?,
            },
            ("Form:Concat", 1) => DocKind::Concat {
                inlines: self
                    .list(id, n, 0, C::Inline)?
                    .into_iter()
                    .map(InlineRef)
                    .collect(),
            },
            ("Form:Ruby", 2) => DocKind::Ruby {
                base: InlineRef(self.node(id, n, 0, C::Inline)?),
                reading: InlineRef(self.node(id, n, 1, C::Inline)?),
            },
            ("Form:Anno", 2) => DocKind::Anno {
                base: InlineRef(self.node(id, n, 0, C::Inline)?),
                notes: self
                    .list(id, n, 1, C::Inline)?
                    .into_iter()
                    .map(InlineRef)
                    .collect(),
            },
            ("Form:InlineMath", 1) => DocKind::InlineMath {
                syntax: self.embed(id, n, 0, EmbedKind::InlineMath)?,
            },
            ("Form:DisplayMath", 1) => DocKind::DisplayMath {
                syntax: self.embed(id, n, 0, EmbedKind::DisplayMath)?,
            },
            ("Form:CircuitFigure", 2) => DocKind::CircuitFigure {
                caption: SentenceRef(self.node(id, n, 0, C::Sentence)?),
                syntax: self.embed(id, n, 1, EmbedKind::CircuitFigure)?,
            },
            ("Form:Code", 1) => DocKind::Code {
                syntax: self.embed(id, n, 0, EmbedKind::Code)?,
            },
            ("Form:Anchor", 2) => DocKind::Anchor {
                id: self.text(id, n, 0, "Builtin:Name")?,
                label: InlineRef(self.node(id, n, 1, C::Inline)?),
            },
            ("Form:Reference", 2) => DocKind::Reference {
                target: self.text(id, n, 0, "Builtin:Name")?,
                label: InlineRef(self.node(id, n, 1, C::Inline)?),
            },
            ("Form:Emphasis", 1) => DocKind::Emphasis {
                inline: InlineRef(self.node(id, n, 0, C::Inline)?),
            },
            ("Form:Strong", 1) => DocKind::Strong {
                inline: InlineRef(self.node(id, n, 0, C::Inline)?),
            },
            ("Form:Break", 0) => DocKind::Break,
            ("Form:Row", 1) => DocKind::Row {
                cells: self
                    .list(id, n, 0, C::Sentence)?
                    .into_iter()
                    .map(SentenceRef)
                    .collect(),
            },
            ("Form:Table", 3) => {
                let refs = self.list(id, n, 0, C::Alignment)?;
                let mut columns = Vec::new();
                for r in refs {
                    let DocKind::Alignment { alignment } = self.nodes[r as usize].kind else {
                        return Err(LowerError::Operand { node: id, field: 0 });
                    };
                    push(&mut columns, alignment, self.b)?;
                }
                let DocKind::OptionalRow { row: header } =
                    self.auxiliary(id, n, 1, C::OptionalRow)?
                else {
                    return Err(LowerError::Operand { node: id, field: 1 });
                };
                DocKind::Table {
                    columns,
                    header,
                    rows: self
                        .list(id, n, 2, C::Row)?
                        .into_iter()
                        .map(RowRef)
                        .collect(),
                }
            }
            ("Form:List", 2) => {
                let DocKind::ListStyle { style: kind } = self.auxiliary(id, n, 0, C::ListStyle)?
                else {
                    return Err(LowerError::Operand { node: id, field: 0 });
                };
                DocKind::List {
                    kind,
                    items: self
                        .list(id, n, 1, C::ListItem)?
                        .into_iter()
                        .map(ListItemRef)
                        .collect(),
                }
            }
            ("Form:ListItem", 2) => {
                let DocKind::Check { checked } = self.auxiliary(id, n, 0, C::Check)? else {
                    return Err(LowerError::Operand { node: id, field: 0 });
                };
                DocKind::ListItem {
                    checked,
                    body: BodyRef(self.node(id, n, 1, C::Body)?),
                }
            }
            ("Form:Link", 2) => {
                let DocKind::Target { target } = self.auxiliary(id, n, 0, C::Target)? else {
                    return Err(LowerError::Operand { node: id, field: 0 });
                };
                DocKind::Link {
                    target,
                    label: InlineRef(self.node(id, n, 1, C::Inline)?),
                }
            }
            ("Form:InlineCode", 1) => DocKind::InlineCode {
                text: self.text(id, n, 0, "Builtin:Text")?,
            },
            ("Form:RawCode", 2) => DocKind::RawCode {
                language_hint: self.optional_text(id, n, 0)?,
                text: self.text(id, n, 1, "Builtin:Text")?,
            },
            ("Form:Image", 3) => {
                let DocKind::Asset { asset } = self.auxiliary(id, n, 0, C::Asset)? else {
                    return Err(LowerError::Operand { node: id, field: 0 });
                };
                let alt = SentenceRef(self.node(id, n, 1, C::Sentence)?);
                let DocKind::OptionalSentence { sentence: caption } =
                    self.auxiliary(id, n, 2, C::OptionalSentence)?
                else {
                    return Err(LowerError::Operand { node: id, field: 2 });
                };
                DocKind::Image {
                    asset,
                    alt,
                    caption,
                }
            }
            ("Form:InlineImage", 2) => {
                let DocKind::Asset { asset } = self.auxiliary(id, n, 0, C::Asset)? else {
                    return Err(LowerError::Operand { node: id, field: 0 });
                };
                DocKind::InlineImage {
                    asset,
                    alt: SentenceRef(self.node(id, n, 1, C::Sentence)?),
                }
            }
            ("Form:AlignmentDefault", 0) => DocKind::Alignment {
                alignment: Alignment::Default,
            },
            ("Form:AlignmentLeft", 0) => DocKind::Alignment {
                alignment: Alignment::Left,
            },
            ("Form:AlignmentCenter", 0) => DocKind::Alignment {
                alignment: Alignment::Center,
            },
            ("Form:AlignmentRight", 0) => DocKind::Alignment {
                alignment: Alignment::Right,
            },
            ("Form:Unordered", 0) => DocKind::ListStyle {
                style: ListKind::Unordered,
            },
            ("Form:Ordered", 1) => DocKind::ListStyle {
                style: ListKind::Ordered {
                    start: self.nat(id, n, 0)?,
                },
            },
            ("Form:CheckNone", 0) => DocKind::Check { checked: None },
            ("Form:Checked", 0) => DocKind::Check {
                checked: Some(true),
            },
            ("Form:Unchecked", 0) => DocKind::Check {
                checked: Some(false),
            },
            ("Form:NoRow", 0) => DocKind::OptionalRow { row: None },
            ("Form:SomeRow", 1) => DocKind::OptionalRow {
                row: Some(RowRef(self.node(id, n, 0, C::Row)?)),
            },
            ("Form:NoSentence", 0) => DocKind::OptionalSentence { sentence: None },
            ("Form:SomeSentence", 1) => DocKind::OptionalSentence {
                sentence: Some(SentenceRef(self.node(id, n, 0, C::Sentence)?)),
            },
            ("Form:NoText", 0) => DocKind::OptionalText { text: None },
            ("Form:SomeText", 1) => DocKind::OptionalText {
                text: Some(self.text(id, n, 0, "Builtin:Text")?),
            },
            ("Form:PageTarget", 2) => DocKind::Target {
                target: LinkTarget::Page {
                    page: self.text(id, n, 0, "Builtin:Text")?,
                    fragment: self.optional_text(id, n, 1)?,
                },
            },
            ("Form:RelativeTarget", 2) => DocKind::Target {
                target: LinkTarget::Relative {
                    path: self.text(id, n, 0, "Builtin:Text")?,
                    fragment: self.optional_text(id, n, 1)?,
                },
            },
            ("Form:ExternalTarget", 1) => DocKind::Target {
                target: LinkTarget::External {
                    uri: self.text(id, n, 0, "Builtin:Text")?,
                },
            },
            ("Form:AssetRef", 2) => DocKind::Asset {
                asset: AssetRef {
                    id: self.text(id, n, 0, "Builtin:Text")?,
                    digest: self.digest(id, n, 1)?,
                },
            },
            _ => return Err(LowerError::Unsupported { node: id }),
        };
        self.b.charge(Resource::Nodes, 1)?;
        let index = self.nodes.len() as u64;
        let span = n
            .cover
            .as_ref()
            .map(|s| super::span(s, self.b))
            .transpose()?;
        push(
            &mut self.nodes,
            DocNode {
                kind,
                origin: Some(n.origin),
                span,
            },
            self.b,
        )?;
        self.mapping[id.0 as usize] = Some(Mapped::Node(index));
        Ok(())
    }
}
