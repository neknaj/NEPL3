use super::*;
use crate::binding::{BindingStage, OccurrenceStage, StageId};
use nepl3_core::facts::{EntityId, OccurrenceId, ScopeId};
macro_rules! id {
    ($ty:ident,$owner:ident,$name:literal) => {
        impl Value for $ty {
            fn value<C: FoundationValueCodec>(
                &self,
                s: &Schemas<'_>,
                _: &mut C,
                b: &mut Budget,
            ) -> Result<NdfValue, PortableError<C::Error>> {
                record(s.$owner, $name, [NdfValue::U64(self.0)], b)
            }
            fn read<C: FoundationValueCodec>(
                v: &NdfValue,
                s: &Schemas<'_>,
                c: &mut C,
                b: &mut Budget,
            ) -> Result<Self, PortableError<C::Error>> {
                Ok(Self(u64::read(
                    &fields(v, s.$owner, $name, 1)?[0],
                    s,
                    c,
                    b,
                )?))
            }
        }
    };
}
id!(StageId, engine, "StageId");
id!(ScopeId, foundation, "ScopeId");
id!(EntityId, foundation, "EntityId");
id!(OccurrenceId, foundation, "OccurrenceId");
impl Value for BindingStage {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let previous = match self.previous {
            Some(v) => Some(v.value(s, c, b)?),
            None => None,
        };
        record(
            s.engine,
            "BindingStage",
            [
                self.scope.value(s, c, b)?,
                super::super::facts::optional(previous, b)?,
                self.introduced.value(s, c, b)?,
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
        let f = fields(v, s.engine, "BindingStage", 3)?;
        Ok(Self {
            scope: ScopeId::read(&f[0], s, c, b)?,
            previous: match &f[1] {
                NdfValue::None => None,
                NdfValue::Some(v) => Some(StageId::read(v, s, c, b)?),
                _ => return Err(PortableError::Shape),
            },
            introduced: Vec::<EntityId>::read(&f[2], s, c, b)?,
        })
    }
}
impl Value for OccurrenceStage {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            s.engine,
            "OccurrenceStage",
            [
                self.occurrence.value(s, c, b)?,
                self.stage.value(s, c, b)?,
                self.namespace_stage.value(s, c, b)?,
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
        let f = fields(v, s.engine, "OccurrenceStage", 3)?;
        Ok(Self {
            occurrence: OccurrenceId::read(&f[0], s, c, b)?,
            stage: StageId::read(&f[1], s, c, b)?,
            namespace_stage: StageId::read(&f[2], s, c, b)?,
        })
    }
}
fn values<T: Value, C: FoundationValueCodec>(
    items: &[T],
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let mut out = Vec::new();
    for item in items {
        push(&mut out, item.value(s, c, b)?, b)?;
    }
    Ok(NdfValue::List(out))
}
pub(super) fn data_value<C: FoundationValueCodec>(
    data: &Data<'_>,
    complete: bool,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let facts = match data.facts {
        Some(v) => Some(c.encode_fact_set(v, b).map_err(boundary)?),
        None => None,
    };
    let facts = if complete {
        facts.ok_or(PortableError::Shape)?
    } else {
        super::super::facts::optional(facts, b)?
    };
    record(
        s.engine,
        if complete {
            "BindingAnalysis"
        } else {
            "BindingProgress"
        },
        [
            facts,
            c.encode_sources(data.sources, b).map_err(boundary)?,
            c.encode_mappings(data.maps, b).map_err(boundary)?,
            values(data.stages, s, c, b)?,
            values(data.occurrences, s, c, b)?,
            values(data.open_inputs, s, c, b)?,
            values(data.exports, s, c, b)?,
        ],
        b,
    )
}
pub(super) fn data_from<C: FoundationValueCodec>(
    v: &NdfValue,
    complete: bool,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<BindingProgress, PortableError<C::Error>> {
    let f = fields(
        v,
        s.engine,
        if complete {
            "BindingAnalysis"
        } else {
            "BindingProgress"
        },
        7,
    )?;
    let facts = if complete {
        Some(c.decode_fact_set(&f[0], b).map_err(boundary)?)
    } else {
        match &f[0] {
            NdfValue::None => None,
            NdfValue::Some(v) => Some(c.decode_fact_set(v, b).map_err(boundary)?),
            _ => return Err(PortableError::Shape),
        }
    };
    let sources = c.decode_sources(&f[1], b).map_err(boundary)?;
    // Mapping positions must resolve in the explicit fact/report source union.
    let empty = BindingProgress {
        facts,
        sources,
        source_maps: Vec::new(),
        stages: Vec::new(),
        occurrence_stages: Vec::new(),
        open_inputs: Vec::new(),
        exports: Vec::new(),
    };
    let store = check::source_closure(&Data::from(&empty), b, c.source_admission())?;
    let mut local = c.scoped(&store);
    Ok(BindingProgress {
        facts: empty.facts,
        sources: empty.sources,
        source_maps: local.decode_mappings(&f[2], b).map_err(boundary)?,
        stages: Vec::<BindingStage>::read(&f[3], s, &mut local, b)?,
        occurrence_stages: Vec::<OccurrenceStage>::read(&f[4], s, &mut local, b)?,
        open_inputs: Vec::<OccurrenceId>::read(&f[5], s, &mut local, b)?,
        exports: Vec::<EntityId>::read(&f[6], s, &mut local, b)?,
    })
}
