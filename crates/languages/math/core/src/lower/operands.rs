use super::*;
use alloc::string::String;
use nepl3_core::value::NdfValue;
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
}
