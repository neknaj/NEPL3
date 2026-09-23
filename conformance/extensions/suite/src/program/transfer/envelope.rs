//! One schema-checked transport unit for a plan and its occurrence source heads.
use super::*;

/// Immutable, admitted packet. Source ownership stays with the receiving host;
/// decoding never grants source access from information supplied by a provider.
pub struct Received {
    value: nepl3_wire::StructuralValue,
    heads: Vec<Option<Span>>,
}

impl Received {
    pub fn program<'a>(
        &'a self,
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<Program<'a>, Error> {
        let (plan, _) = parts(self.value.value())?;
        let [NdfValue::List(nodes)] = plan.fields.as_slice() else {
            return Err(Error::Shape);
        };
        // The private immutable packet has passed schema and domain admission.
        Checked { nodes }.program(&self.heads, sources, budget)
    }
}

fn parts(value: &NdfValue) -> Result<(&Record, &[NdfValue]), Error> {
    let NdfValue::Record(record) = value else {
        return Err(Error::Shape);
    };
    let [NdfValue::Record(plan), NdfValue::List(heads)] = record.fields.as_slice() else {
        return Err(Error::Shape);
    };
    Ok((plan, heads))
}

fn reserve<T>(values: &mut Vec<T>, count: usize, budget: &mut Budget) -> Result<(), Error> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::AllocationUnits, bytes as u64)?;
    values
        .try_reserve_exact(count)
        .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
    Ok(())
}

pub fn encode(
    program: &Program<'_>,
    schema: &SchemaRef,
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Vec<u8>, Error> {
    let TypedValue::Record(plan) = super::encode(program, schema, budget)? else {
        return Err(Error::Shape);
    };
    let mut heads = Vec::new();
    reserve(&mut heads, program.nodes().len(), budget)?;
    for node in program.nodes() {
        budget.charge(Resource::Work, 1)?;
        heads.push(match node.head {
            None => NdfValue::None,
            Some(span) => {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<NdfValue>() as u64,
                )?;
                let bytes = nepl3_wire::source::encode_span(span, foundation, registry, budget)
                    .map_err(Error::Wire)?;
                NdfValue::Some(Box::new(NdfValue::Bytes(bytes)))
            }
        });
    }
    budget.charge(
        Resource::AllocationUnits,
        (schema.package.len() + "Envelope".len() + 2 * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    let value = NdfValue::Record(Record {
        schema: schema.clone(),
        kind: "Envelope".into(),
        fields: vec![NdfValue::Record(plan), NdfValue::List(heads)],
    });
    nepl3_wire::encode_checked(&value, &named("Envelope"), registry, budget).map_err(Error::Wire)
}

pub fn decode(
    bytes: &[u8],
    schema: &SchemaRef,
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    budget: &mut Budget,
) -> Result<Received, Error> {
    let value = nepl3_wire::decode_checked(bytes, &named("Envelope"), registry, budget)
        .map_err(Error::Wire)?;
    let NdfValue::Record(record) = value.value() else {
        return Err(Error::Shape);
    };
    budget.charge(
        Resource::Work,
        (schema.package.len() + record.schema.package.len() + 48) as u64,
    )?;
    if record.schema != *schema || record.kind != "Envelope" {
        return Err(Error::Shape);
    }
    let (plan, encoded_heads) = parts(value.value())?;
    let checked = validate_record(plan, budget)?;
    if encoded_heads.len() != checked.nodes.len() {
        return Err(Error::Reference);
    }
    let mut heads = Vec::new();
    reserve(&mut heads, encoded_heads.len(), budget)?;
    for head in encoded_heads {
        budget.charge(Resource::Work, 1)?;
        heads.push(match head {
            NdfValue::None => None,
            NdfValue::Some(value) => {
                let NdfValue::Bytes(bytes) = value.as_ref() else {
                    return Err(Error::Shape);
                };
                Some(
                    nepl3_wire::source::decode_span(bytes, foundation, registry, sources, budget)
                        .map_err(Error::Wire)?,
                )
            }
            _ => return Err(Error::Shape),
        });
    }
    Ok(Received { value, heads })
}
