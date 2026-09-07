use super::*;
use alloc::string::String;
use nepl3_core::{source::Digest, value::NdfValue};
impl Adapter<'_, '_> {
    pub(super) fn child(
        &self,
        id: NodeRef,
        node: &SyntaxNode,
        field: usize,
    ) -> Result<NodeRef, LowerError> {
        match node.fields.get(field) {
            Some(FieldValue::Child(child)) => Ok(*child),
            _ => Err(LowerError::Operand { node: id, field }),
        }
    }
    fn syntax(&self, id: NodeRef) -> Result<&SyntaxNode, LowerError> {
        self.checked
            .bundle()
            .nodes
            .get(usize::try_from(id.0).map_err(|_| SyntaxError::Reference)?)
            .ok_or(SyntaxError::Reference.into())
    }
    pub(super) fn node(
        &self,
        id: NodeRef,
        n: &SyntaxNode,
        f: usize,
        c: Category,
    ) -> Result<u64, LowerError> {
        let child = self.child(id, n, f)?;
        let Some(Mapped::Node(index)) = self.mapping.get(child.0 as usize).copied().flatten()
        else {
            return Err(LowerError::Operand { node: id, field: f });
        };
        if !crate::check::edges::accepts(&self.nodes[index as usize].kind, c) {
            return Err(LowerError::Operand { node: id, field: f });
        }
        Ok(index)
    }
    pub(super) fn list(
        &mut self,
        id: NodeRef,
        n: &SyntaxNode,
        f: usize,
        c: Category,
    ) -> Result<Vec<u64>, LowerError> {
        let mut current = self.child(id, n, f)?;
        let mut out = Vec::new();
        loop {
            self.b.charge(Resource::Work, 1)?;
            let node = self.syntax(current)?;
            if !node.kind.starts_with("List:") {
                return Err(LowerError::Operand { node: id, field: f });
            }
            if node.fields.is_empty() {
                break;
            }
            if node.fields.len() != 2 {
                return Err(LowerError::Operand { node: id, field: f });
            }
            let index = self.node(current, node, 0, c)?;
            current = self.child(current, node, 1)?;
            push(&mut out, index, self.b)?;
        }
        Ok(out)
    }
    pub(super) fn text(
        &mut self,
        id: NodeRef,
        n: &SyntaxNode,
        f: usize,
        kind: &str,
    ) -> Result<String, LowerError> {
        let child = self.child(id, n, f)?;
        let bundle = self.checked.bundle();
        let node = bundle
            .nodes
            .get(child.0 as usize)
            .ok_or(SyntaxError::Reference)?;
        self.b
            .charge(Resource::Work, (node.kind.len() + kind.len()) as u64)?;
        if node.kind != kind {
            return Err(LowerError::Operand { node: id, field: f });
        }
        let token = node
            .token
            .and_then(|t| bundle.tokens.get(t.0 as usize))
            .ok_or(LowerError::Operand { node: id, field: f })?;
        let NdfValue::Text(text) = &token.payload else {
            return Err(LowerError::Operand { node: id, field: f });
        };
        self.b.charge(Resource::Work, text.len() as u64)?;
        self.b
            .charge(Resource::AllocationUnits, text.len() as u64)?;
        Ok(text.clone())
    }
    pub(super) fn nat(&mut self, id: NodeRef, n: &SyntaxNode, f: usize) -> Result<u64, LowerError> {
        let child = self.child(id, n, f)?;
        let bundle = self.checked.bundle();
        let node = bundle
            .nodes
            .get(child.0 as usize)
            .ok_or(SyntaxError::Reference)?;
        self.b.charge(Resource::Work, node.kind.len() as u64 + 12)?;
        if node.kind != "Builtin:Nat" {
            return Err(LowerError::Operand { node: id, field: f });
        }
        let token = node
            .token
            .and_then(|t| bundle.tokens.get(t.0 as usize))
            .ok_or(LowerError::Operand { node: id, field: f })?;
        let NdfValue::Integer(value) = &token.payload else {
            return Err(LowerError::Operand { node: id, field: f });
        };
        self.b
            .charge(Resource::Work, value.as_bigint().bits().div_ceil(8) + 1)?;
        if value.is_negative() || value.as_bigint().bits() > 64 {
            return Err(LowerError::NaturalRange { node: child });
        }
        // The checked <=64-bit magnitude has at most one digit; zero has none.
        Ok(value.as_bigint().iter_u64_digits().sum())
    }
    pub(super) fn auxiliary(
        &mut self,
        id: NodeRef,
        n: &SyntaxNode,
        f: usize,
        c: Category,
    ) -> Result<DocKind, LowerError> {
        let index = self.node(id, n, f, c)?;
        let kind = &self.nodes[index as usize].kind;
        let bytes = crate::copy::kind_bytes(kind);
        self.b.charge(Resource::Work, bytes + 1)?;
        self.b.charge(
            Resource::AllocationUnits,
            bytes + core::mem::size_of::<DocKind>() as u64,
        )?;
        Ok(kind.clone())
    }
    pub(super) fn optional_text(
        &mut self,
        id: NodeRef,
        n: &SyntaxNode,
        f: usize,
    ) -> Result<Option<String>, LowerError> {
        let DocKind::OptionalText { text } = self.auxiliary(id, n, f, Category::OptionalText)?
        else {
            return Err(LowerError::Operand { node: id, field: f });
        };
        Ok(text)
    }
    pub(super) fn digest(
        &mut self,
        id: NodeRef,
        n: &SyntaxNode,
        f: usize,
    ) -> Result<Option<Digest>, LowerError> {
        let Some(text) = self.optional_text(id, n, f)? else {
            return Ok(None);
        };
        let child = self.child(id, n, f)?;
        if text.len() != 64 {
            return Err(LowerError::AssetDigest { node: child });
        }
        self.b.charge(Resource::Work, 64)?;
        let mut bytes = [0u8; 32];
        fn digit(b: u8) -> Option<u8> {
            match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                b'A'..=b'F' => Some(b - b'A' + 10),
                _ => None,
            }
        }
        for (pair, out) in text.as_bytes().chunks_exact(2).zip(&mut bytes) {
            *out = (digit(pair[0]).ok_or(LowerError::AssetDigest { node: child })? << 4)
                | digit(pair[1]).ok_or(LowerError::AssetDigest { node: child })?;
        }
        Ok(Some(Digest(bytes)))
    }
    pub(super) fn embed(
        &mut self,
        id: NodeRef,
        n: &SyntaxNode,
        f: usize,
        kind: EmbedKind,
        admission: &mut SourceAdmission,
    ) -> Result<EmbedRef, LowerError> {
        let child = self.child(id, n, f)?;
        let node = self
            .checked
            .bundle()
            .nodes
            .get(child.0 as usize)
            .ok_or(SyntaxError::Reference)?;
        let accepted = match kind {
            EmbedKind::InlineMath | EmbedKind::DisplayMath => node.kind == "Form:MathGuest",
            EmbedKind::CircuitFigure => node.kind == "Form:CircuitGuest",
            EmbedKind::Code | EmbedKind::Guest => matches!(
                node.kind.as_str(),
                "Form:MathGuest" | "Form:CircuitGuest" | "Form:GrammarGuest" | "Form:DocGuest"
            ),
        };
        if !accepted {
            return Err(LowerError::Operand { node: id, field: f });
        }
        let [FieldValue::Foreign(foreign)] = node.fields.as_slice() else {
            return Err(LowerError::Operand {
                node: child,
                field: 0,
            });
        };
        let closure =
            ForeignClosure::capture(foreign, self.checked, self.registry, self.b, admission)?;
        let index = EmbedRef(self.embeds.len() as u64);
        push(&mut self.embeds, DocEmbed { kind, closure }, self.b)?;
        Ok(index)
    }
}
