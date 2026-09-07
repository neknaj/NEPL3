use super::*;
impl Builder<'_, '_> {
    pub(super) fn block(&mut self, j: Job, k: &DocKind) -> Result<bool, RenderError> {
        use DocKind::*;
        let Job {
            node,
            parent,
            level,
        } = j;
        match k {
            Body { blocks } => {
                for child in blocks.iter().rev() {
                    self.job(child.0, parent, level)?;
                }
            }
            Paragraph { items } => {
                let div = self.element(Some(parent), node, HtmlTag::Div)?;
                self.class(div, "nepl-paragraph")?;
                let mut run = None;
                let start = self.jobs.len();
                for child in items {
                    self.b.charge(Resource::Work, 1)?;
                    let kind = &self.prepared.document.value.nodes[child.0 as usize].kind;
                    let target = if matches!(kind, Sentence { .. } | Parallel { .. }) {
                        match run {
                            Some(p) => p,
                            None => {
                                let p = self.element(Some(div), node, HtmlTag::P)?;
                                run = Some(p);
                                p
                            }
                        }
                    } else {
                        run = None;
                        // Reserve a block wrapper now, preserving order against
                        // sentence runs before descendant jobs are executed.
                        self.element(Some(div), child.0, HtmlTag::Div)?
                    };
                    self.job(child.0, target, level)?;
                }
                self.b
                    .charge(Resource::Work, (self.jobs.len() - start) as u64)?;
                self.jobs[start..].reverse();
            }
            Section { id, title, body } => {
                let e = self.element(Some(parent), node, HtmlTag::Section)?;
                let id = hex_id(id, self.b)?;
                self.attr(e, HtmlAttribute::Id { value: id })?;
                let next = level
                    .checked_add(1)
                    .ok_or(RenderError::OutputDepth { node })?;
                self.job(body.0, e, next)?;
                self.heading(e, node, title.0, next)?;
            }
            List { kind, items } => {
                let e = self.element(
                    Some(parent),
                    node,
                    match kind {
                        ListKind::Unordered => HtmlTag::Ul,
                        ListKind::Ordered { .. } => HtmlTag::Ol,
                    },
                )?;
                if let ListKind::Ordered { start } = kind {
                    self.attr(e, HtmlAttribute::Start { value: *start })?;
                }
                for child in items.iter().rev() {
                    self.job(child.0, e, level)?;
                }
            }
            ListItem { checked, body } => {
                let e = self.element(Some(parent), node, HtmlTag::Li)?;
                if let Some(checked) = checked {
                    let mark = self.element(Some(e), node, HtmlTag::Span)?;
                    self.class(mark, "nepl-checkbox")?;
                    let label = copy(if *checked { "checked" } else { "unchecked" }, self.b)?;
                    self.attr(
                        mark,
                        HtmlAttribute::Role {
                            value: HtmlRole::Img,
                        },
                    )?;
                    self.attr(mark, HtmlAttribute::AriaLabel { value: label })?;
                    self.text(mark, node, if *checked { "☑ " } else { "☐ " })?;
                }
                self.job(body.0, e, level)?;
            }
            Table {
                columns,
                header,
                rows,
            } => {
                let table = self.element(Some(parent), node, HtmlTag::Table)?;
                let start = self.jobs.len();
                if let Some(header) = header {
                    let head = self.element(Some(table), node, HtmlTag::Thead)?;
                    self.row(header.0, head, columns, true, level)?;
                }
                if !rows.is_empty() {
                    let body = self.element(Some(table), node, HtmlTag::Tbody)?;
                    for row in rows {
                        self.b.charge(Resource::Work, 1)?;
                        self.row(row.0, body, columns, false, level)?;
                    }
                }
                self.b
                    .charge(Resource::Work, (self.jobs.len() - start) as u64)?;
                self.jobs[start..].reverse();
            }
            RawCode {
                language_hint,
                text,
            } => {
                let e = self.element(Some(parent), node, HtmlTag::Figure)?;
                if let Some(hint) = language_hint {
                    let caption = self.element(Some(e), node, HtmlTag::Figcaption)?;
                    self.text(caption, node, hint)?;
                }
                let pre = self.element(Some(e), node, HtmlTag::Pre)?;
                let code = self.element(Some(pre), node, HtmlTag::Code)?;
                self.text(code, node, text)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    fn row(
        &mut self,
        node: u64,
        parent: u64,
        columns: &[Alignment],
        header: bool,
        level: u64,
    ) -> Result<(), RenderError> {
        let DocKind::Row { cells } = &self.prepared.document.value.nodes[node as usize].kind else {
            return Err(RenderError::InternalShape);
        };
        let row = self.element(Some(parent), node, HtmlTag::Tr)?;
        for (cell, alignment) in cells.iter().zip(columns) {
            self.b.charge(Resource::Work, 1)?;
            let td = self.element(
                Some(row),
                node,
                if header { HtmlTag::Th } else { HtmlTag::Td },
            )?;
            if header {
                self.attr(
                    td,
                    HtmlAttribute::Scope {
                        value: CellScope::Col,
                    },
                )?;
            }
            match alignment {
                Alignment::Default => {}
                Alignment::Left => self.class(td, "nepl-align-left")?,
                Alignment::Center => self.class(td, "nepl-align-center")?,
                Alignment::Right => self.class(td, "nepl-align-right")?,
            }
            self.job(cell.0, td, level)?;
        }
        Ok(())
    }
}
