use alloc::string::String;
use nepl3_core::budget::{Budget, Resource, StopReason};
#[derive(Default)]
pub(super) struct Output {
    pub text: String,
}
impl Output {
    pub fn precharged(&mut self, text: &str, b: &mut Budget) -> Result<(), StopReason> {
        b.charge(Resource::Work, text.len() as u64)?;
        // Geometric capacity growth, with relocation charged before reserve.
        let required = self
            .text
            .len()
            .checked_add(text.len())
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        if required > self.text.capacity() {
            let capacity = required.max(self.text.capacity().saturating_mul(2)).max(16);
            b.charge(
                Resource::AllocationUnits,
                (capacity - self.text.capacity()) as u64,
            )?;
            b.charge(Resource::Work, self.text.len() as u64)?;
            self.text
                .try_reserve_exact(capacity - self.text.len())
                .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        }
        self.text.push_str(text);
        Ok(())
    }
    fn append(&mut self, text: &str, b: &mut Budget) -> Result<(), StopReason> {
        b.charge(Resource::OutputBytes, text.len() as u64)?;
        self.precharged(text, b)
    }
    pub fn start(&mut self, b: &mut Budget) -> Result<(), StopReason> {
        if !self.text.is_empty() {
            self.append(" ", b)?;
        }
        Ok(())
    }
    pub fn atom(&mut self, text: &str, b: &mut Budget) -> Result<(), StopReason> {
        self.start(b)?;
        self.append(text, b)
    }
    pub fn quoted(&mut self, text: &str, b: &mut Budget) -> Result<(), StopReason> {
        self.start(b)?;
        self.append("\"", b)?;
        for ch in text.chars() {
            b.charge(Resource::Work, 1)?;
            match ch {
                '\\' => self.append("\\\\", b)?,
                '"' => self.append("\\\"", b)?,
                '\n' => self.append("\\n", b)?,
                '\r' => self.append("\\r", b)?,
                '\t' => self.append("\\t", b)?,
                _ => {
                    let mut bytes = [0; 4];
                    self.append(ch.encode_utf8(&mut bytes), b)?;
                }
            }
        }
        self.append("\"", b)
    }
}
