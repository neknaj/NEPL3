//! Flat, schema-checked transport of a dependency plan to native callbacks.
//! The host retains source spans in Program; node indices identify occurrences.
use super::{Instruction, Program};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{
        FieldDescriptor, NamedType, SchemaDescriptor, SchemaError, SchemaRegistry, TypeDescriptor,
        TypeRef, TypeShape, VariantDescriptor,
    },
    value::{NdfValue, Record, SchemaRef, TypedValue, Variant},
};

pub const PACKAGE: &str = "org.example.composition.plan";

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Schema(SchemaError),
    Shape,
    Reference,
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

fn named(name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: PACKAGE.into(),
        revision: 1,
        name: name.into(),
    })
}

/// Fixed descriptor. References are occurrence indices; semantic validation
/// additionally requires a single connected, postorder tree.
pub fn descriptor(budget: &mut Budget) -> Result<SchemaDescriptor, StopReason> {
    budget.charge(Resource::Work, 4096)?;
    budget.charge(Resource::AllocationUnits, 16384)?;
    let field = |name: &str, ty| FieldDescriptor {
        name: name.into(),
        ty,
    };
    let variants = ["Natural", "Neg", "Add", "Mul", "Framed", "Frame"]
        .into_iter()
        .map(|name| VariantDescriptor {
            name: name.into(),
            fields: match name {
                "Natural" => vec![field("value", TypeDescriptor::Integer)],
                "Add" | "Mul" => vec![
                    field("left", TypeDescriptor::U64),
                    field("right", TypeDescriptor::U64),
                ],
                _ => vec![field("child", TypeDescriptor::U64)],
            },
        })
        .collect();
    Ok(SchemaDescriptor {
        package: PACKAGE.into(),
        revision: 1,
        types: vec![
            NamedType {
                name: "Node".into(),
                constraints: vec![],
                shape: TypeShape::Variant { variants },
            },
            NamedType {
                name: "Plan".into(),
                constraints: vec![],
                shape: TypeShape::Record {
                    fields: vec![field(
                        "nodes",
                        TypeDescriptor::List(Box::new(named("Node"))),
                    )],
                },
            },
        ],
        operations: vec![],
    })
}

/// Encode the existing typed plan, preserving occurrence order and boundaries.
/// No source strings or syntax reconstruction participate in this conversion.
pub fn encode(
    plan: &Program<'_>,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<TypedValue, Error> {
    if plan.root.0.checked_add(1) != Some(plan.nodes.len()) {
        return Err(Error::Reference);
    }
    let count = plan.nodes.len();
    let fixed = count
        .checked_mul(
            core::mem::size_of::<NdfValue>()
                + core::mem::size_of::<Variant>()
                + 2 * core::mem::size_of::<NdfValue>()
                + schema.package.len()
                + 32,
        )
        .and_then(|n| n.checked_add(core::mem::size_of::<Record>() + schema.package.len() + 64))
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::AllocationUnits, fixed as u64)?;
    budget.charge(Resource::Work, fixed as u64)?;
    budget.charge(Resource::Nodes, count as u64)?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(count)
        .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
    let id = |id: super::ValueId| {
        u64::try_from(id.0)
            .map(NdfValue::U64)
            .map_err(|_| Error::Reference)
    };
    for node in &plan.nodes {
        let (name, fields) = match &node.instruction {
            Instruction::Natural(value) => {
                let bytes = value.as_bigint().bits() / 8 + 1;
                budget.charge(Resource::Work, bytes)?;
                budget.charge(Resource::AllocationUnits, bytes.saturating_add(32))?;
                ("Natural", vec![NdfValue::Integer((*value).clone())])
            }
            Instruction::Neg(child) => ("Neg", vec![id(*child)?]),
            Instruction::Add(left, right) => ("Add", vec![id(*left)?, id(*right)?]),
            Instruction::Mul(left, right) => ("Mul", vec![id(*left)?, id(*right)?]),
            Instruction::Framed(child) => ("Framed", vec![id(*child)?]),
            Instruction::Frame(child) => ("Frame", vec![id(*child)?]),
        };
        nodes.push(NdfValue::Variant(Variant {
            schema: schema.clone(),
            type_name: "Node".into(),
            variant: name.into(),
            fields,
        }));
    }
    Ok(TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "Plan".into(),
        fields: vec![NdfValue::List(nodes)],
    }))
}

/// Borrowed proof binds a complete schema identity to a connected plan. Each
/// child has one parent, permitting a unique request ID per occurrence.
pub struct Checked<'a> {
    nodes: &'a [NdfValue],
}
impl<'a> Checked<'a> {
    pub fn nodes(&self) -> &'a [NdfValue] {
        self.nodes
    }
}

pub fn validate<'a>(
    value: &'a TypedValue,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Checked<'a>, Error> {
    budget.poll()?;
    let TypedValue::Record(record) = value else {
        return Err(Error::Shape);
    };
    budget.charge(
        Resource::Work,
        (schema.package.len() + record.schema.package.len() + 48) as u64,
    )?;
    if record.schema != *schema || record.kind != "Plan" {
        return Err(Error::Shape);
    }
    // Schema validation precedes domain checks and all indexing.
    registry
        .validate_typed(value, budget)
        .map_err(Error::Schema)?;
    let [NdfValue::List(nodes)] = record.fields.as_slice() else {
        return Err(Error::Shape);
    };
    if nodes.is_empty() {
        return Err(Error::Shape);
    }
    budget.charge(Resource::AllocationUnits, nodes.len() as u64)?;
    budget.charge(Resource::Work, nodes.len() as u64)?;
    let mut used = Vec::new();
    used.try_reserve_exact(nodes.len())
        .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
    used.resize(nodes.len(), false);
    for (index, node) in nodes.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        let NdfValue::Variant(node) = node else {
            return Err(Error::Shape);
        };
        for child in &node.fields {
            if node.variant == "Natural" {
                let NdfValue::Integer(value) = child else {
                    return Err(Error::Shape);
                };
                if value.is_negative() {
                    return Err(Error::Shape);
                }
                continue;
            }
            let NdfValue::U64(child) = child else {
                return Err(Error::Shape);
            };
            let child = usize::try_from(*child).map_err(|_| Error::Reference)?;
            if child >= index || used[child] {
                return Err(Error::Reference);
            }
            let NdfValue::Variant(target) = &nodes[child] else {
                return Err(Error::Shape);
            };
            // Only Framed enters Frame; Frame returns to MiniExpr.
            if (target.variant == "Frame") != (node.variant == "Framed") {
                return Err(Error::Reference);
            }
            used[child] = true;
        }
    }
    if used[..nodes.len() - 1].iter().any(|used| !used) {
        return Err(Error::Reference);
    }
    let Some(NdfValue::Variant(root)) = nodes.last() else {
        return Err(Error::Shape);
    };
    if root.variant == "Frame" {
        return Err(Error::Reference);
    }
    Ok(Checked { nodes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use external_hello_language::{budget, error};
    use nepl3_core::value::Integer;

    #[test]
    fn disconnected_negative_and_wrong_wire_type_are_rejected() -> Result<(), String> {
        let descriptor = descriptor(&mut budget()).map_err(error)?;
        let identity = descriptor.reference(&mut budget()).map_err(error)?;
        let mut registry = SchemaRegistry::default();
        registry
            .register(identity.clone(), descriptor, &mut budget())
            .map_err(error)?;
        registry.finalize(&mut budget()).map_err(error)?;
        let leaf = |value| {
            NdfValue::Variant(Variant {
                schema: identity.clone(),
                type_name: "Node".into(),
                variant: "Natural".into(),
                fields: vec![value],
            })
        };
        let plan = |nodes| {
            TypedValue::Record(Record {
                schema: identity.clone(),
                kind: "Plan".into(),
                fields: vec![NdfValue::List(nodes)],
            })
        };
        let single = plan(vec![leaf(NdfValue::Integer(Integer::from(0_i64)))]);
        assert_eq!(
            validate(&single, &identity, &registry, &mut budget())
                .map_err(error)?
                .nodes()
                .len(),
            1
        );
        let disconnected = plan(vec![
            leaf(NdfValue::Integer(Integer::from(1_i64))),
            leaf(NdfValue::Integer(Integer::from(2_i64))),
        ]);
        registry
            .validate_typed(&disconnected, &mut budget())
            .map_err(error)?;
        assert!(matches!(
            validate(&disconnected, &identity, &registry, &mut budget()),
            Err(Error::Reference)
        ));
        let negative = plan(vec![leaf(NdfValue::Integer(Integer::from(-1_i64)))]);
        registry
            .validate_typed(&negative, &mut budget())
            .map_err(error)?;
        assert!(matches!(
            validate(&negative, &identity, &registry, &mut budget()),
            Err(Error::Shape)
        ));
        let wrong_type = plan(vec![leaf(NdfValue::U64(0))]);
        assert!(matches!(
            validate(&wrong_type, &identity, &registry, &mut budget()),
            Err(Error::Schema(_))
        ));
        assert!(matches!(
            validate(&plan(vec![]), &identity, &registry, &mut budget()),
            Err(Error::Shape)
        ));
        Ok(())
    }
}
