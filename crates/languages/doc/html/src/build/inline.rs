use super::*;
impl Builder<'_, '_> {
    pub(super) fn inline(&mut self, j: Job, k: &DocKind) -> Result<(), RenderError> {
        use DocKind::*;
        let Job {
            node,
            parent,
            level,
            ..
        } = j;
        match k {
            Sentence { syntax } => {
                let e = self.element(Some(parent), node, HtmlTag::Span)?;
                self.foreign_job(node, *syntax, e, level)?;
            }
            Anchor { id, label } | Reference { target: id, label } => {
                let anchor = matches!(k, Anchor { .. });
                let e = self.element(
                    Some(parent),
                    node,
                    if anchor { HtmlTag::Span } else { HtmlTag::A },
                )?;
                let id = hex_id(id, self.b)?;
                self.attr(
                    e,
                    if anchor {
                        HtmlAttribute::Id { value: id }
                    } else {
                        HtmlAttribute::Href {
                            value: HtmlHref::Fragment { id },
                        }
                    },
                )?;
                self.foreign_job(node, *label, e, level)?;
            }
            Link { label, .. } => {
                let mut href = None;
                for (index, target) in self.links {
                    self.b.charge(Resource::Work, 1)?;
                    if *index == node {
                        href = Some(match target {
                            HtmlHref::BetweenArtifacts {
                                source,
                                target,
                                fragment,
                            } => HtmlHref::BetweenArtifacts {
                                source: copy(source, self.b)?,
                                target: copy(target, self.b)?,
                                fragment: fragment.as_ref().map(|s| copy(s, self.b)).transpose()?,
                            },
                            HtmlHref::External { uri } => HtmlHref::External {
                                uri: copy(uri, self.b)?,
                            },
                            _ => return Err(RenderError::InternalShape),
                        });
                        break;
                    }
                }
                let value = href.ok_or(RenderError::InternalShape)?;
                let e = self.element(Some(parent), node, HtmlTag::A)?;
                self.attr(e, HtmlAttribute::Href { value })?;
                self.foreign_job(node, *label, e, level)?;
            }
            Parallel { variants } => {
                let e = self.element(Some(parent), node, HtmlTag::Span)?;
                let class = match self.prepared.options.parallel {
                    ParallelMode::Rows => "nepl-parallel-rows",
                    ParallelMode::Columns => "nepl-parallel-columns",
                    ParallelMode::Single { .. } => "nepl-parallel-single",
                };
                self.class(e, class)?;
                // Use the distinct generated occurrence for a shared Doc node.
                let mut id = copy("g-", self.b)?;
                self.b.charge(Resource::Work, 16)?;
                self.b.charge(Resource::AllocationUnits, 32)?;
                for byte in e.to_be_bytes() {
                    id.push(b"0123456789abcdef"[(byte >> 4) as usize] as char);
                    id.push(b"0123456789abcdef"[(byte & 15) as usize] as char);
                }
                self.attr(e, HtmlAttribute::DataGroup { value: id })?;
                if let Some(selected) = self.prepared.selections[node as usize] {
                    self.job(selected.0, e, level)?;
                } else {
                    for child in variants.iter().rev() {
                        self.job(child.0, e, level)?;
                    }
                }
            }
            Variant { language, sentence } => {
                let e = self.element(Some(parent), node, HtmlTag::Span)?;
                let language = copy(language, self.b)?;
                self.attr(e, HtmlAttribute::Lang { value: language })?;
                self.job(sentence.0, e, level)?;
            }
            _ => return Err(RenderError::InternalShape),
        }
        Ok(())
    }
}
