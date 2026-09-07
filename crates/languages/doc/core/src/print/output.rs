//! Budgeted surface output. Text escaping follows the shared Text reader's
//! explicit escape set; compact literal punctuation adds its own escapes.
use alloc::string::String;
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Default)]
pub(super) struct Output {
    pub text: String,
}
impl Output {
    pub fn append(&mut self, value: &str, b: &mut Budget) -> Result<(), StopReason> {
        b.charge(Resource::Work, value.len() as u64)?;
        b.charge(Resource::OutputBytes, value.len() as u64)?;
        b.charge(Resource::AllocationUnits, value.len() as u64)?;
        self.text.push_str(value);
        Ok(())
    }
    pub fn start(&mut self, b: &mut Budget) -> Result<(), StopReason> {
        if !self.text.is_empty() {
            self.append(" ", b)?;
        }
        Ok(())
    }
    pub fn atom(&mut self, value: &str, b: &mut Budget) -> Result<(), StopReason> {
        self.start(b)?;
        self.append(value, b)
    }
    pub fn quoted(&mut self, value: &str, b: &mut Budget) -> Result<(), StopReason> {
        self.start(b)?;
        self.append("\"", b)?;
        self.escaped(value, false, b)?;
        self.append("\"", b)
    }
    pub fn escaped(
        &mut self,
        value: &str,
        literal: bool,
        b: &mut Budget,
    ) -> Result<(), StopReason> {
        for ch in value.chars() {
            b.charge(Resource::Work, 1)?;
            match ch {
                '\\' => self.append("\\\\", b)?,
                '"' => self.append("\\\"", b)?,
                '\n' => self.append("\\n", b)?,
                '\r' => self.append("\\r", b)?,
                '\t' => self.append("\\t", b)?,
                '[' | ']' | '{' | '}' | '/' if literal => {
                    self.append("\\", b)?;
                    let mut bytes = [0; 4];
                    self.append(ch.encode_utf8(&mut bytes), b)?;
                }
                _ => {
                    let mut bytes = [0; 4];
                    self.append(ch.encode_utf8(&mut bytes), b)?;
                }
            }
        }
        Ok(())
    }
}
