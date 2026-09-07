use super::*;
use crate::facts::{FactsHeader, FactsPhase};

impl Value for FactsHeader {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            s.engine,
            "FactsHeader",
            [
                self.group.value(s, c, b)?,
                self.provider.value(s, c, b)?,
                self.target.value(s, c, b)?,
                self.entities.value(s, c, b)?,
                self.exports.value(s, c, b)?,
            ],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, s.engine, "FactsHeader", 5)?;
        Ok(Self {
            group: Value::read(&f[0], s, c, b)?,
            provider: Value::read(&f[1], s, c, b)?,
            target: Value::read(&f[2], s, c, b)?,
            entities: Value::read(&f[3], s, c, b)?,
            exports: Value::read(&f[4], s, c, b)?,
        })
    }
}
impl Value for FactsPhase {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Ordinary => variant(s.engine, "FactsPhase", "Ordinary", [], b),
            Self::Header { group } => {
                variant(s.engine, "FactsPhase", "Header", [group.value(s, c, b)?], b)
            }
            Self::Body { header } => {
                variant(s.engine, "FactsPhase", "Body", [header.value(s, c, b)?], b)
            }
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let (case, f) = parts(v, s.engine, "FactsPhase")?;
        match (case, f) {
            ("Ordinary", []) => Ok(Self::Ordinary),
            ("Header", [group]) => Ok(Self::Header {
                group: Value::read(group, s, c, b)?,
            }),
            ("Body", [header]) => {
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<FactsHeader>() as u64,
                )?;
                Ok(Self::Body {
                    header: Box::new(FactsHeader::read(header, s, c, b)?),
                })
            }
            _ => Err(PortableError::Shape),
        }
    }
}
