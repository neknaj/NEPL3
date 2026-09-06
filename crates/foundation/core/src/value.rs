//! NDF logical values. Numeric constructors preserve canonical wire invariants.
pub mod decimal;
use crate::source::Digest;
use alloc::{boxed::Box, string::String, vec::Vec};
use num_bigint::{BigInt, BigUint, Sign};
use num_integer::Integer as _;
use num_traits::{One, Zero};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SchemaRef {
    pub package: String,
    pub revision: u64,
    pub digest: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRef {
    pub schema: SchemaRef,
    pub name: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KindRef {
    pub schema: SchemaRef,
    pub local_kind: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumberError {
    LeadingZero,
    NegativeZero,
    ZeroDenominator,
    NotReduced,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Integer(BigInt);
impl Integer {
    pub fn from_bigint(value: BigInt) -> Self {
        Self(value)
    }
    pub fn as_bigint(&self) -> &BigInt {
        &self.0
    }
    pub fn from_canonical(negative: bool, magnitude: &[u8]) -> Result<Self, NumberError> {
        if magnitude.first() == Some(&0) {
            return Err(NumberError::LeadingZero);
        }
        if negative && magnitude.is_empty() {
            return Err(NumberError::NegativeZero);
        }
        Ok(Self(BigInt::from_bytes_be(
            if negative { Sign::Minus } else { Sign::Plus },
            magnitude,
        )))
    }
    pub fn canonical_parts(&self) -> (bool, Vec<u8>) {
        if self.0.is_zero() {
            return (false, Vec::new());
        }
        let (sign, bytes) = self.0.to_bytes_be();
        (sign == Sign::Minus, bytes)
    }
    pub fn is_negative(&self) -> bool {
        self.0.sign() == Sign::Minus
    }
}
impl From<i64> for Integer {
    fn from(value: i64) -> Self {
        Self(BigInt::from(value))
    }
}
impl From<u64> for Integer {
    fn from(value: u64) -> Self {
        Self(BigInt::from(value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rational {
    numerator: Integer,
    denominator: BigUint,
}
impl Rational {
    /// Native constructor normalizes signs and reduces arbitrary precision inputs.
    pub fn new(numerator: BigInt, denominator: BigInt) -> Result<Self, NumberError> {
        if denominator.is_zero() {
            return Err(NumberError::ZeroDenominator);
        }
        let numerator = if denominator.sign() == Sign::Minus {
            -numerator
        } else {
            numerator
        };
        let denominator = denominator.magnitude();
        let gcd = numerator.magnitude().gcd(denominator);
        Ok(Self {
            numerator: Integer(numerator / BigInt::from(gcd.clone())),
            denominator: denominator / gcd,
        })
    }
    /// Wire boundary constructor rejects noncanonical values instead of silently repairing them.
    pub fn from_canonical(numerator: Integer, denominator: &[u8]) -> Result<Self, NumberError> {
        if denominator.is_empty() {
            return Err(NumberError::ZeroDenominator);
        }
        if denominator.first() == Some(&0) {
            return Err(NumberError::LeadingZero);
        }
        let denominator = BigUint::from_bytes_be(denominator);
        if numerator.0.magnitude().gcd(&denominator) != BigUint::one() {
            return Err(NumberError::NotReduced);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }
    pub fn numerator(&self) -> &Integer {
        &self.numerator
    }
    pub fn denominator(&self) -> &BigUint {
        &self.denominator
    }
    pub fn denominator_bytes(&self) -> Vec<u8> {
        self.denominator.to_bytes_be()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    pub schema: SchemaRef,
    pub kind: String,
    pub fields: Vec<NdfValue>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variant {
    pub schema: SchemaRef,
    pub type_name: String,
    pub variant: String,
    pub fields: Vec<NdfValue>,
}
pub enum NdfValue {
    Unit,
    Bool(bool),
    U64(u64),
    Integer(Integer),
    Rational(Rational),
    Text(String),
    Bytes(Vec<u8>),
    List(Vec<NdfValue>),
    None,
    Some(Box<NdfValue>),
    Record(Record),
    Variant(Variant),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NdfScalar {
    Unit,
    Bool(bool),
    U64(u64),
    Integer(Integer),
    Rational(Rational),
    Text(String),
    Bytes(Vec<u8>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypedValue {
    Record(Record),
    Variant(Variant),
}
impl TypedValue {
    pub fn clone_with_budget(
        &self,
        budget: &mut crate::budget::Budget,
    ) -> Result<Self, crate::budget::StopReason> {
        use crate::budget::Resource;
        let (schema, name, variant, fields) = match self {
            Self::Record(v) => (&v.schema, &v.kind, None, &v.fields),
            Self::Variant(v) => (&v.schema, &v.type_name, Some(&v.variant), &v.fields),
        };
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Self>() as u64
                + (schema.package.len() + name.len() + variant.map_or(0, |v| v.len())) as u64,
        )?;
        let fields = fields
            .iter()
            .map(|value| value.clone_with_budget(budget))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(match variant {
            None => Self::Record(Record {
                schema: schema.clone(),
                kind: name.clone(),
                fields,
            }),
            Some(variant) => Self::Variant(Variant {
                schema: schema.clone(),
                type_name: name.clone(),
                variant: variant.clone(),
                fields,
            }),
        })
    }
}

impl NdfValue {
    /// Charges logical work and copied payload storage before cloning an external value.
    pub fn clone_with_budget(
        &self,
        budget: &mut crate::budget::Budget,
    ) -> Result<Self, crate::budget::StopReason> {
        self.charge_clone(budget)?;
        Ok(self.clone())
    }
    /// Precharges a subsequent clone, including iterative traversal storage.
    pub fn charge_clone(
        &self,
        budget: &mut crate::budget::Budget,
    ) -> Result<(), crate::budget::StopReason> {
        use crate::budget::Resource;
        budget.charge(
            Resource::AllocationUnits,
            2 * core::mem::size_of::<(&Self, u64)>() as u64,
        )?;
        let mut pending = alloc::vec![(self, 1u64)];
        while let Some((value, depth)) = pending.pop() {
            budget.observe_depth(depth)?;
            budget.charge(Resource::Work, 1)?;
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Self>() as u64,
            )?;
            let mut children: &[NdfValue] = &[];
            let extra = match value {
                Self::Text(v) => v.len() as u64,
                Self::Bytes(v) => v.len() as u64,
                Self::Integer(v) => v.as_bigint().bits().div_ceil(8),
                Self::Rational(v) => {
                    v.numerator().as_bigint().bits().div_ceil(8)
                        + v.denominator().bits().div_ceil(8)
                }
                Self::List(v) => {
                    children = v;
                    0
                }
                Self::Some(v) => {
                    children = core::slice::from_ref(v.as_ref());
                    0
                }
                Self::Record(v) => {
                    children = &v.fields;
                    (v.schema.package.len() + v.kind.len()) as u64
                }
                Self::Variant(v) => {
                    children = &v.fields;
                    (v.schema.package.len() + v.type_name.len() + v.variant.len()) as u64
                }
                _ => 0,
            };
            budget.charge(Resource::AllocationUnits, extra)?;
            for child in children {
                budget.charge(
                    Resource::AllocationUnits,
                    2 * core::mem::size_of::<(&Self, u64)>() as u64,
                )?;
                pending.push((
                    child,
                    depth
                        .checked_add(1)
                        .ok_or(crate::budget::StopReason::DepthLimit)?,
                ));
            }
        }
        Ok(())
    }
    fn detach_children(&mut self, pending: &mut Vec<Self>) {
        match self {
            Self::Some(value) => pending.push(core::mem::replace(value.as_mut(), Self::Unit)),
            Self::List(values) => pending.append(values),
            Self::Record(value) => pending.append(&mut value.fields),
            Self::Variant(value) => pending.append(&mut value.fields),
            _ => {}
        }
    }
}
impl Drop for NdfValue {
    fn drop(&mut self) {
        // Cleanup cannot return a logical budget error; use a heap worklist instead of
        // a call-stack frame per untrusted nesting level, including partial decoder values.
        let mut pending = Vec::new();
        self.detach_children(&mut pending);
        while let Some(mut value) = pending.pop() {
            value.detach_children(&mut pending);
        }
    }
}

impl Clone for NdfValue {
    fn clone(&self) -> Self {
        fn shallow(value: &NdfValue) -> NdfValue {
            match value {
                NdfValue::Unit => NdfValue::Unit,
                NdfValue::Bool(v) => NdfValue::Bool(*v),
                NdfValue::U64(v) => NdfValue::U64(*v),
                NdfValue::Integer(v) => NdfValue::Integer(v.clone()),
                NdfValue::Rational(v) => NdfValue::Rational(v.clone()),
                NdfValue::Text(v) => NdfValue::Text(v.clone()),
                NdfValue::Bytes(v) => NdfValue::Bytes(v.clone()),
                NdfValue::None => NdfValue::None,
                NdfValue::Some(_) => NdfValue::Some(Box::new(NdfValue::Unit)),
                NdfValue::List(v) => NdfValue::List((0..v.len()).map(|_| NdfValue::Unit).collect()),
                NdfValue::Record(v) => NdfValue::Record(Record {
                    schema: v.schema.clone(),
                    kind: v.kind.clone(),
                    fields: (0..v.fields.len()).map(|_| NdfValue::Unit).collect(),
                }),
                NdfValue::Variant(v) => NdfValue::Variant(Variant {
                    schema: v.schema.clone(),
                    type_name: v.type_name.clone(),
                    variant: v.variant.clone(),
                    fields: (0..v.fields.len()).map(|_| NdfValue::Unit).collect(),
                }),
            }
        }
        let mut result = Self::Unit;
        let mut pending = alloc::vec![(self, &mut result)];
        while let Some((source, target)) = pending.pop() {
            *target = shallow(source);
            match (source, target) {
                (Self::Some(source), Self::Some(target)) => {
                    pending.push((source.as_ref(), target.as_mut()))
                }
                (Self::List(source), Self::List(target)) => {
                    pending.extend(source.iter().zip(target.iter_mut()))
                }
                (Self::Record(source), Self::Record(target)) => {
                    pending.extend(source.fields.iter().zip(target.fields.iter_mut()))
                }
                (Self::Variant(source), Self::Variant(target)) => {
                    pending.extend(source.fields.iter().zip(target.fields.iter_mut()))
                }
                _ => {}
            }
        }
        result
    }
}
impl PartialEq for NdfValue {
    fn eq(&self, other: &Self) -> bool {
        let mut pending = alloc::vec![(self, other)];
        while let Some((a, b)) = pending.pop() {
            match (a, b) {
                (Self::Unit, Self::Unit) | (Self::None, Self::None) => {}
                (Self::Bool(a), Self::Bool(b)) if a == b => {}
                (Self::U64(a), Self::U64(b)) if a == b => {}
                (Self::Integer(a), Self::Integer(b)) if a == b => {}
                (Self::Rational(a), Self::Rational(b)) if a == b => {}
                (Self::Text(a), Self::Text(b)) if a == b => {}
                (Self::Bytes(a), Self::Bytes(b)) if a == b => {}
                (Self::Some(a), Self::Some(b)) => pending.push((a, b)),
                (Self::List(a), Self::List(b)) if a.len() == b.len() => {
                    pending.extend(a.iter().zip(b))
                }
                (Self::Record(a), Self::Record(b))
                    if a.schema == b.schema
                        && a.kind == b.kind
                        && a.fields.len() == b.fields.len() =>
                {
                    pending.extend(a.fields.iter().zip(&b.fields))
                }
                (Self::Variant(a), Self::Variant(b))
                    if a.schema == b.schema
                        && a.type_name == b.type_name
                        && a.variant == b.variant
                        && a.fields.len() == b.fields.len() =>
                {
                    pending.extend(a.fields.iter().zip(&b.fields))
                }
                _ => return false,
            }
        }
        true
    }
}
impl Eq for NdfValue {}
/// Container Debug deliberately prints only its header/count. Wire is the full tree representation.
impl core::fmt::Debug for NdfValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unit => f.write_str("Unit"),
            Self::None => f.write_str("None"),
            Self::Some(_) => f.write_str("Some(..)"),
            Self::Bool(v) => f.debug_tuple("Bool").field(v).finish(),
            Self::U64(v) => f.debug_tuple("U64").field(v).finish(),
            Self::Integer(v) => f.debug_tuple("Integer").field(v).finish(),
            Self::Rational(v) => f.debug_tuple("Rational").field(v).finish(),
            Self::Text(v) => f.debug_tuple("Text").field(v).finish(),
            Self::Bytes(v) => f.debug_tuple("Bytes").field(v).finish(),
            Self::List(v) => f.debug_struct("List").field("len", &v.len()).finish(),
            Self::Record(v) => f
                .debug_struct("Record")
                .field("schema", &v.schema)
                .field("kind", &v.kind)
                .field("fields", &v.fields.len())
                .finish(),
            Self::Variant(v) => f
                .debug_struct("Variant")
                .field("schema", &v.schema)
                .field("type", &v.type_name)
                .field("variant", &v.variant)
                .field("fields", &v.fields.len())
                .finish(),
        }
    }
}
