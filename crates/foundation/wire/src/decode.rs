use crate::WireError;
use alloc::{boxed::Box, string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    source::Digest,
    value::{Integer, NdfValue, Rational, Record, SchemaRef, Variant},
};

struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl<'a> Decoder<'a> {
    fn take(&mut self, length: usize, budget: &mut Budget) -> Result<&'a [u8], WireError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(WireError::InvalidLength)?;
        let bytes = self
            .bytes
            .get(self.cursor..end)
            .ok_or(WireError::UnexpectedEnd)?;
        budget.charge(Resource::Work, length as u64)?;
        self.cursor = end;
        Ok(bytes)
    }
    fn head(&mut self, major: u8, budget: &mut Budget) -> Result<u64, WireError> {
        let first = self.take(1, budget)?[0];
        if first >> 5 != major {
            return Err(WireError::InvalidType);
        }
        let additional = first & 31;
        if additional < 24 {
            return Ok(additional.into());
        }
        let (size, minimum) = match additional {
            24 => (1, 24),
            25 => (2, 256),
            26 => (4, 65_536),
            27 => (8, 4_294_967_296),
            _ => return Err(WireError::InvalidType),
        };
        let mut bytes = [0; 8];
        bytes[8 - size..].copy_from_slice(self.take(size, budget)?);
        let value = u64::from_be_bytes(bytes);
        if value < minimum {
            return Err(WireError::NonCanonical);
        }
        Ok(value)
    }
    fn raw(&mut self, major: u8, budget: &mut Budget) -> Result<&'a [u8], WireError> {
        let length =
            usize::try_from(self.head(major, budget)?).map_err(|_| WireError::InvalidLength)?;
        self.take(length, budget)
    }
    fn text(&mut self, budget: &mut Budget) -> Result<String, WireError> {
        let bytes = self.raw(3, budget)?;
        let text = core::str::from_utf8(bytes).map_err(|_| WireError::InvalidUtf8)?;
        budget.charge(Resource::AllocationUnits, bytes.len() as u64)?;
        Ok(text.into())
    }
    fn boolean(&mut self, budget: &mut Budget) -> Result<bool, WireError> {
        match self.take(1, budget)?[0] {
            0xf4 => Ok(false),
            0xf5 => Ok(true),
            _ => Err(WireError::InvalidType),
        }
    }
    fn integer_payload(&mut self, budget: &mut Budget) -> Result<Integer, WireError> {
        let negative = self.boolean(budget)?;
        let magnitude = self.raw(2, budget)?;
        budget.charge(Resource::AllocationUnits, magnitude.len() as u64)?;
        Ok(Integer::from_canonical(negative, magnitude)?)
    }
    fn integer(&mut self, budget: &mut Budget) -> Result<Integer, WireError> {
        if self.head(4, budget)? != 3 || self.head(0, budget)? != 3 {
            return Err(WireError::InvalidType);
        }
        self.integer_payload(budget)
    }
    fn schema(&mut self, budget: &mut Budget) -> Result<SchemaRef, WireError> {
        if self.head(4, budget)? != 3 {
            return Err(WireError::InvalidLength);
        }
        let package = self.text(budget)?;
        let revision = self.head(0, budget)?;
        let digest = self.raw(2, budget)?;
        let digest: [u8; 32] = digest.try_into().map_err(|_| WireError::InvalidLength)?;
        Ok(SchemaRef {
            package,
            revision,
            digest: Digest(digest),
        })
    }

    fn list_length(&mut self, budget: &mut Budget) -> Result<u64, WireError> {
        let length = self.head(4, budget)?;
        if length > ((self.bytes.len() - self.cursor) / 2) as u64 {
            return Err(WireError::UnexpectedEnd);
        }
        let allocation = length
            .checked_mul(core::mem::size_of::<NdfValue>() as u64)
            .ok_or(WireError::InvalidLength)?;
        budget.charge(Resource::AllocationUnits, allocation)?;
        Ok(length)
    }
    fn node(&mut self, budget: &mut Budget) -> Result<Partial, WireError> {
        budget.charge(Resource::Nodes, 1)?;
        let length = self.head(4, budget)?;
        let tag = self.head(0, budget)?;
        let expected = match tag {
            0 | 8 => 1,
            1 | 2 | 5 | 6 | 7 | 9 => 2,
            3 | 4 => 3,
            10 => 4,
            11 => 5,
            _ => return Err(WireError::InvalidTag),
        };
        if length != expected {
            return Err(WireError::InvalidLength);
        }
        let value = match tag {
            0 => NdfValue::Unit,
            1 => NdfValue::Bool(self.boolean(budget)?),
            2 => NdfValue::U64(self.head(0, budget)?),
            3 => NdfValue::Integer(self.integer_payload(budget)?),
            4 => {
                let numerator = self.integer(budget)?;
                let denominator = self.raw(2, budget)?;
                budget.charge(Resource::AllocationUnits, denominator.len() as u64)?;
                let length = numerator
                    .as_bigint()
                    .bits()
                    .div_ceil(8)
                    .checked_add(denominator.len() as u64)
                    .ok_or(WireError::InvalidLength)?;
                budget.charge(
                    Resource::Work,
                    length.checked_mul(length).ok_or(WireError::InvalidLength)?,
                )?;
                NdfValue::Rational(Rational::from_canonical(numerator, denominator)?)
            }
            5 => NdfValue::Text(self.text(budget)?),
            6 => {
                let bytes = self.raw(2, budget)?;
                budget.charge(Resource::AllocationUnits, bytes.len() as u64)?;
                NdfValue::Bytes(bytes.to_vec())
            }
            7 => return self.frame(FrameKind::List, budget),
            8 => NdfValue::None,
            9 => {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<NdfValue>() as u64,
                )?;
                return Ok(Partial::Container(Frame {
                    kind: FrameKind::Some,
                    remaining: 1,
                    values: Vec::new(),
                }));
            }
            10 => {
                let schema = self.schema(budget)?;
                let kind = self.text(budget)?;
                return self.frame(FrameKind::Record { schema, kind }, budget);
            }
            11 => {
                let schema = self.schema(budget)?;
                let type_name = self.text(budget)?;
                let variant = self.text(budget)?;
                return self.frame(
                    FrameKind::Variant {
                        schema,
                        type_name,
                        variant,
                    },
                    budget,
                );
            }
            _ => return Err(WireError::InvalidTag),
        };
        Ok(Partial::Value(value))
    }
    fn frame(&mut self, kind: FrameKind, budget: &mut Budget) -> Result<Partial, WireError> {
        let remaining = self.list_length(budget)?;
        let frame = Frame {
            kind,
            remaining,
            values: Vec::new(),
        };
        if remaining == 0 {
            Ok(Partial::Value(frame.finish()?))
        } else {
            Ok(Partial::Container(frame))
        }
    }
    fn value(&mut self, budget: &mut Budget) -> Result<NdfValue, WireError> {
        let mut frames: Vec<Frame> = Vec::new();
        loop {
            budget.observe_depth(
                (frames.len() as u64)
                    .checked_add(1)
                    .ok_or(nepl3_core::budget::StopReason::DepthLimit)?,
            )?;
            let mut completed = match self.node(budget)? {
                Partial::Value(value) => value,
                Partial::Container(frame) => {
                    budget.charge(
                        Resource::AllocationUnits,
                        core::mem::size_of::<Frame>() as u64,
                    )?;
                    frames.push(frame);
                    continue;
                }
            };
            loop {
                let Some(frame) = frames.last_mut() else {
                    return Ok(completed);
                };
                frame.values.push(completed);
                frame.remaining = frame
                    .remaining
                    .checked_sub(1)
                    .ok_or(WireError::InvalidLength)?;
                if frame.remaining != 0 {
                    break;
                }
                completed = frames.pop().ok_or(WireError::InvalidLength)?.finish()?;
            }
        }
    }
}

enum FrameKind {
    List,
    Some,
    Record {
        schema: SchemaRef,
        kind: String,
    },
    Variant {
        schema: SchemaRef,
        type_name: String,
        variant: String,
    },
}
struct Frame {
    kind: FrameKind,
    remaining: u64,
    values: Vec<NdfValue>,
}
impl Frame {
    fn finish(mut self) -> Result<NdfValue, WireError> {
        if self.remaining != 0 {
            return Err(WireError::UnexpectedEnd);
        }
        Ok(match self.kind {
            FrameKind::List => NdfValue::List(self.values),
            FrameKind::Some => {
                NdfValue::Some(Box::new(self.values.pop().ok_or(WireError::InvalidLength)?))
            }
            FrameKind::Record { schema, kind } => NdfValue::Record(Record {
                schema,
                kind,
                fields: self.values,
            }),
            FrameKind::Variant {
                schema,
                type_name,
                variant,
            } => NdfValue::Variant(Variant {
                schema,
                type_name,
                variant,
                fields: self.values,
            }),
        })
    }
}
enum Partial {
    Value(NdfValue),
    Container(Frame),
}

pub(super) fn decode(bytes: &[u8], budget: &mut Budget) -> Result<NdfValue, WireError> {
    let mut decoder = Decoder { bytes, cursor: 0 };
    let value = decoder.value(budget)?;
    if decoder.cursor != bytes.len() {
        return Err(WireError::TrailingData);
    }
    Ok(value)
}
