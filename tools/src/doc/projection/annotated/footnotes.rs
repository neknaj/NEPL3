//! Deferred definitions retain typed inline nodes and original nesting depth.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct PendingFootnote<'a> {
    pub notes: &'a [InlineRef],
    pub depth: u64,
}
impl Annotated<'_, '_> {
    pub(super) fn footnote_marker(&mut self, number: u64) -> Result<String, Error> {
        // The prefix and every decimal u64 fit within this fixed allowance.
        self.plain.budget.charge(Resource::Work, 64)?;
        self.plain.budget.charge(Resource::AllocationUnits, 64)?;
        Ok(format!("[^nepl3-anno-{number}]"))
    }
    pub(super) fn finish_footnotes(&mut self) -> Result<(), Error> {
        let mut index = 0;
        // Nested Anno nodes can append definitions while rendering a note.
        // Inspection has already rejected cycles; cumulative budget still
        // bounds repeated DAG occurrences and the complete output.
        while index < self.footnotes.len() {
            self.plain.budget.charge(Resource::Work, 1)?;
            let PendingFootnote { notes, depth } = self.footnotes[index];
            let marker = self.footnote_marker(index as u64 + 1)?;
            self.plain.emit(&marker)?;
            if notes.len() == 1 {
                self.plain.emit(": ")?;
                self.inline_at(&[notes[0].0], Some("    "), false, depth)?;
            } else {
                self.plain.emit(":\n")?;
                for (part, note) in notes.iter().enumerate() {
                    self.plain.budget.charge(Resource::Work, 64)?;
                    self.plain.budget.charge(Resource::AllocationUnits, 64)?;
                    let prefix = format!("    {}. ", part + 1);
                    self.plain.emit(&prefix)?;
                    self.inline_at(&[note.0], Some("        "), false, depth)?;
                    self.plain.emit("\n")?;
                }
            }
            self.plain.emit("\n\n")?;
            index += 1;
        }
        Ok(())
    }
}
