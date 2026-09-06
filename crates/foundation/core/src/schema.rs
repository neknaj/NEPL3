//! Typed descriptors, deterministic identities and structural boundary checking.
//! Domain constraint IDs remain obligations for the domain checker.
pub mod foundation;
use crate::{
    budget::{Budget, Resource, StopReason},
    source::Digest,
    value::{NdfValue, SchemaRef},
};
use alloc::{boxed::Box, collections::BTreeSet, string::String, vec::Vec};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRef {
    pub package: String,
    pub revision: u64,
    pub name: String,
}
pub enum TypeDescriptor {
    Unit,
    Bool,
    U64,
    Integer,
    Natural,
    Rational,
    Text,
    Bytes,
    Bytes32,
    NdfValue,
    NdfScalar,
    TypedValue,
    List(Box<Self>),
    Option(Box<Self>),
    Named(TypeRef),
}
impl TypeDescriptor {
    /// Accounts for copied boxes, the iterative clone stack, and symbolic-name payloads.
    pub fn clone_with_budget(&self, budget: &mut Budget) -> Result<Self, StopReason> {
        let mut ty = self;
        let mut depth = 1u64;
        loop {
            budget.observe_depth(depth)?;
            budget.charge(Resource::Work, 1)?;
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Self>() as u64 + 1,
            )?;
            match ty {
                Self::List(inner) | Self::Option(inner) => {
                    ty = inner;
                    depth = depth.checked_add(1).ok_or(StopReason::DepthLimit)?;
                }
                Self::Named(reference) => {
                    budget.charge(
                        Resource::AllocationUnits,
                        (reference.name.len() + reference.package.len()) as u64,
                    )?;
                    break;
                }
                _ => break,
            }
        }
        Ok(self.clone())
    }

    fn detach_child(&mut self) -> Option<Self> {
        match self {
            Self::List(inner) | Self::Option(inner) => {
                Some(core::mem::replace(inner.as_mut(), Self::Unit))
            }
            _ => None,
        }
    }
}
impl Drop for TypeDescriptor {
    fn drop(&mut self) {
        let mut next = self.detach_child();
        while let Some(mut ty) = next {
            next = ty.detach_child();
        }
    }
}
impl Clone for TypeDescriptor {
    fn clone(&self) -> Self {
        let mut layers = Vec::new();
        let mut source = self;
        let mut result = loop {
            match source {
                Self::List(inner) => {
                    layers.push(true);
                    source = inner;
                }
                Self::Option(inner) => {
                    layers.push(false);
                    source = inner;
                }
                Self::Unit => break Self::Unit,
                Self::Bool => break Self::Bool,
                Self::U64 => break Self::U64,
                Self::Integer => break Self::Integer,
                Self::Natural => break Self::Natural,
                Self::Rational => break Self::Rational,
                Self::Text => break Self::Text,
                Self::Bytes => break Self::Bytes,
                Self::Bytes32 => break Self::Bytes32,
                Self::NdfValue => break Self::NdfValue,
                Self::NdfScalar => break Self::NdfScalar,
                Self::TypedValue => break Self::TypedValue,
                Self::Named(v) => break Self::Named(v.clone()),
            }
        };
        for list in layers.into_iter().rev() {
            result = if list {
                Self::List(Box::new(result))
            } else {
                Self::Option(Box::new(result))
            };
        }
        result
    }
}
impl PartialEq for TypeDescriptor {
    fn eq(&self, other: &Self) -> bool {
        let mut a = self;
        let mut b = other;
        loop {
            match (a, b) {
                (Self::List(x), Self::List(y)) | (Self::Option(x), Self::Option(y)) => {
                    a = x;
                    b = y;
                }
                (Self::Named(x), Self::Named(y)) => return x == y,
                (Self::Unit, Self::Unit)
                | (Self::Bool, Self::Bool)
                | (Self::U64, Self::U64)
                | (Self::Integer, Self::Integer)
                | (Self::Natural, Self::Natural)
                | (Self::Rational, Self::Rational)
                | (Self::Text, Self::Text)
                | (Self::Bytes, Self::Bytes)
                | (Self::Bytes32, Self::Bytes32)
                | (Self::NdfValue, Self::NdfValue)
                | (Self::NdfScalar, Self::NdfScalar)
                | (Self::TypedValue, Self::TypedValue) => return true,
                _ => return false,
            }
        }
    }
}
impl Eq for TypeDescriptor {}
impl core::fmt::Debug for TypeDescriptor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Named(reference) => f.debug_tuple("Named").field(reference).finish(),
            ty => f.write_str(match ty {
                Self::Unit => "Unit",
                Self::Bool => "Bool",
                Self::U64 => "U64",
                Self::Integer => "Integer",
                Self::Natural => "Natural",
                Self::Rational => "Rational",
                Self::Text => "Text",
                Self::Bytes => "Bytes",
                Self::Bytes32 => "Bytes32",
                Self::NdfValue => "NdfValue",
                Self::NdfScalar => "NdfScalar",
                Self::TypedValue => "TypedValue",
                Self::List(_) => "List(..)",
                Self::Option(_) => "Option(..)",
                Self::Named(_) => "Named(..)",
            }),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDescriptor {
    pub name: String,
    pub ty: TypeDescriptor,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDescriptor {
    pub name: String,
    pub fields: Vec<FieldDescriptor>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeShape {
    Record { fields: Vec<FieldDescriptor> },
    Variant { variants: Vec<VariantDescriptor> },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedType {
    pub name: String,
    pub shape: TypeShape,
    pub constraints: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationDescriptor {
    pub name: String,
    pub input: TypeDescriptor,
    pub output: TypeDescriptor,
    pub pure: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaDescriptor {
    pub package: String,
    pub revision: u64,
    pub types: Vec<NamedType>,
    pub operations: Vec<OperationDescriptor>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaError {
    Stopped(StopReason),
    DuplicateName,
    EmptyName,
    DescriptorDepth,
    IdentityMismatch,
    ConflictingSchema,
    UnknownSchema,
    UnknownType,
    WrongType,
    FieldCount,
    UnknownVariant,
    Unfinalized,
}
impl From<StopReason> for SchemaError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}

pub struct CanonicalWriter<'a> {
    bytes: Vec<u8>,
    budget: &'a mut Budget,
}
impl<'a> CanonicalWriter<'a> {
    /// Low-level descriptor writer; callers own structural tokens and canonical key order.
    /// quoted and number encode payloads, while push appends caller-owned JSON syntax.
    pub fn new(budget: &'a mut Budget) -> Self {
        Self {
            bytes: Vec::new(),
            budget,
        }
    }
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub fn push(&mut self, text: &str) -> Result<(), SchemaError> {
        self.budget
            .charge(Resource::AllocationUnits, text.len() as u64)?;
        self.budget.charge(Resource::Work, text.len() as u64)?;
        self.bytes.extend_from_slice(text.as_bytes());
        Ok(())
    }
    pub fn quoted(&mut self, text: &str) -> Result<(), SchemaError> {
        self.push("\"")?;
        for c in text.chars() {
            match c {
                '"' => self.push("\\\"")?,
                '\\' => self.push("\\\\")?,
                '\u{0}'..='\u{1f}' => {
                    let digits = b"0123456789abcdef";
                    let code = c as usize;
                    let bytes = [
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        digits[code >> 4],
                        digits[code & 15],
                    ];
                    let text = core::str::from_utf8(&bytes).map_err(|_| SchemaError::WrongType)?;
                    self.push(text)?;
                }
                _ => {
                    let mut buf = [0; 4];
                    self.push(c.encode_utf8(&mut buf))?;
                }
            }
        }
        self.push("\"")
    }
    pub fn number(&mut self, number: u64) -> Result<(), SchemaError> {
        use alloc::string::ToString;
        self.push(&number.to_string())
    }
    pub fn ty(&mut self, ty: &TypeDescriptor, depth: u64) -> Result<(), SchemaError> {
        if depth > 128 {
            return Err(SchemaError::DescriptorDepth);
        }
        self.budget.observe_depth(depth + 1)?;
        match ty {
            TypeDescriptor::List(inner) | TypeDescriptor::Option(inner) => {
                self.push(if matches!(ty, TypeDescriptor::List(_)) {
                    "{\"list\":"
                } else {
                    "{\"option\":"
                })?;
                self.ty(inner, depth + 1)?;
                self.push("}")
            }
            TypeDescriptor::Named(reference) => {
                if reference.package.is_empty() || reference.name.is_empty() {
                    return Err(SchemaError::EmptyName);
                }
                self.push("{\"named\":{\"name\":")?;
                self.quoted(&reference.name)?;
                self.push(",\"package\":")?;
                self.quoted(&reference.package)?;
                self.push(",\"revision\":")?;
                self.number(reference.revision)?;
                self.push("}}")
            }
            _ => self.quoted(match ty {
                TypeDescriptor::Unit => "Unit",
                TypeDescriptor::Bool => "Bool",
                TypeDescriptor::U64 => "U64",
                TypeDescriptor::Integer => "Integer",
                TypeDescriptor::Natural => "Natural",
                TypeDescriptor::Rational => "Rational",
                TypeDescriptor::Text => "Text",
                TypeDescriptor::Bytes => "Bytes",
                TypeDescriptor::Bytes32 => "Bytes32",
                TypeDescriptor::NdfValue => "NdfValue",
                TypeDescriptor::NdfScalar => "NdfScalar",
                TypeDescriptor::TypedValue => "TypedValue",
                _ => return Err(SchemaError::WrongType),
            }),
        }
    }
    fn fields(&mut self, fields: &[FieldDescriptor]) -> Result<(), SchemaError> {
        let mut names = BTreeSet::new();
        self.push("[")?;
        for (index, field) in fields.iter().enumerate() {
            if field.name.is_empty() {
                return Err(SchemaError::EmptyName);
            }
            if !names.insert(&field.name) {
                return Err(SchemaError::DuplicateName);
            }
            if index != 0 {
                self.push(",")?;
            }
            self.push("[")?;
            self.quoted(&field.name)?;
            self.push(",")?;
            self.ty(&field.ty, 0)?;
            self.push("]")?;
        }
        self.push("]")
    }
}
fn sorted<'a, T>(
    items: &'a [T],
    name: impl Fn(&T) -> &str,
    budget: &mut Budget,
) -> Result<Vec<&'a T>, SchemaError> {
    budget.charge(
        Resource::AllocationUnits,
        (items.len() as u64).saturating_mul(core::mem::size_of::<&T>() as u64),
    )?;
    budget.charge(Resource::Work, items.len() as u64)?;
    let mut result: Vec<_> = items.iter().collect();
    result.sort_by(|a, b| name(a).cmp(name(b)));
    if result.iter().any(|v| name(v).is_empty()) {
        return Err(SchemaError::EmptyName);
    }
    if result.windows(2).any(|pair| name(pair[0]) == name(pair[1])) {
        return Err(SchemaError::DuplicateName);
    }
    Ok(result)
}
impl SchemaDescriptor {
    /// Canonical JSON from spec13; declaration maps sort by names, field arrays retain order.
    pub fn canonical_json(&self, budget: &mut Budget) -> Result<Vec<u8>, SchemaError> {
        if self.package.is_empty() {
            return Err(SchemaError::EmptyName);
        }
        let mut out = CanonicalWriter {
            bytes: Vec::new(),
            budget,
        };
        out.push("{\"operations\":{")?;
        for (index, operation) in sorted(&self.operations, |v| &v.name, out.budget)?
            .iter()
            .enumerate()
        {
            if index != 0 {
                out.push(",")?;
            }
            out.quoted(&operation.name)?;
            out.push(":{\"input\":")?;
            out.ty(&operation.input, 0)?;
            out.push(",\"output\":")?;
            out.ty(&operation.output, 0)?;
            out.push(",\"pure\":")?;
            out.push(if operation.pure { "true}" } else { "false}" })?;
        }
        out.push("},\"package\":")?;
        out.quoted(&self.package)?;
        out.push(",\"revision\":")?;
        out.number(self.revision)?;
        out.push(",\"types\":{")?;
        for (index, ty) in sorted(&self.types, |v| &v.name, out.budget)?
            .iter()
            .enumerate()
        {
            if index != 0 {
                out.push(",")?;
            }
            out.quoted(&ty.name)?;
            out.push(":{\"constraints\":[")?;
            for (index, id) in sorted(&ty.constraints, |v| v.as_str(), out.budget)?
                .iter()
                .enumerate()
            {
                if index != 0 {
                    out.push(",")?;
                }
                out.quoted(id)?;
            }
            out.push("]")?;
            match &ty.shape {
                TypeShape::Record { fields } => {
                    out.push(",\"record\":")?;
                    out.fields(fields)?;
                }
                TypeShape::Variant { variants } => {
                    out.push(",\"variant\":{")?;
                    for (index, variant) in sorted(variants, |v| &v.name, out.budget)?
                        .iter()
                        .enumerate()
                    {
                        if index != 0 {
                            out.push(",")?;
                        }
                        out.quoted(&variant.name)?;
                        out.push(":")?;
                        out.fields(&variant.fields)?;
                    }
                    out.push("}")?;
                }
            }
            out.push("}")?;
        }
        out.push("}}")?;
        Ok(out.bytes)
    }
    pub fn reference(&self, budget: &mut Budget) -> Result<SchemaRef, SchemaError> {
        use sha2::{Digest as _, Sha256};
        let bytes = self.canonical_json(budget)?;
        let mut hasher = Sha256::new();
        hasher.update(b"NEPL3-SCHEMA-1\0");
        hasher.update(bytes);
        Ok(SchemaRef {
            package: self.package.clone(),
            revision: self.revision,
            digest: Digest(hasher.finalize().into()),
        })
    }
}

/// Proof of structural validation only. Schema-owned semantic constraints remain unchecked.
#[derive(Debug)]
pub struct StructuralValue<'a> {
    value: &'a NdfValue,
}
impl<'a> StructuralValue<'a> {
    pub fn value(&self) -> &'a NdfValue {
        self.value
    }
}
#[derive(Debug, Default)]
pub struct SchemaRegistry {
    schemas: Vec<(SchemaRef, SchemaDescriptor)>,
    finalized: bool,
}
impl SchemaRegistry {
    pub fn selected(&self, package: &str, revision: u64) -> Option<&SchemaRef> {
        self.schemas
            .iter()
            .map(|(reference, _)| reference)
            .find(|reference| reference.package == package && reference.revision == revision)
    }
    /// Checks a borrowed typed payload without cloning a possibly deep external value.
    pub fn validate_typed(
        &self,
        value: &crate::value::TypedValue,
        budget: &mut Budget,
    ) -> Result<(), SchemaError> {
        if !self.finalized {
            return Err(SchemaError::Unfinalized);
        }
        let (schema, name) = match value {
            crate::value::TypedValue::Record(v) => (&v.schema, &v.kind),
            crate::value::TypedValue::Variant(v) => (&v.schema, &v.type_name),
        };
        let descriptor = self.descriptor(schema).ok_or(SchemaError::UnknownSchema)?;
        let definition = descriptor
            .types
            .iter()
            .find(|ty| &ty.name == name)
            .ok_or(SchemaError::UnknownType)?;
        let (fields, values) = match (&definition.shape, value) {
            (TypeShape::Record { fields }, crate::value::TypedValue::Record(v)) => {
                (fields, &v.fields)
            }
            (TypeShape::Variant { variants }, crate::value::TypedValue::Variant(v)) => (
                &variants
                    .iter()
                    .find(|variant| variant.name == v.variant)
                    .ok_or(SchemaError::UnknownVariant)?
                    .fields,
                &v.fields,
            ),
            _ => return Err(SchemaError::WrongType),
        };
        if fields.len() != values.len() {
            return Err(SchemaError::FieldCount);
        }
        budget.charge(Resource::Work, 1)?;
        budget.charge(Resource::Nodes, 1)?;
        budget.with_depth(|budget| {
            for (field, value) in fields.iter().zip(values) {
                self.validate(&field.ty, value, budget)?;
            }
            Ok(())
        })
    }
    pub fn is_finalized(&self) -> bool {
        self.finalized
    }
    pub fn register(
        &mut self,
        expected: SchemaRef,
        mut descriptor: SchemaDescriptor,
        budget: &mut Budget,
    ) -> Result<(), SchemaError> {
        if descriptor.reference(budget)? != expected {
            return Err(SchemaError::IdentityMismatch);
        }
        for (reference, _) in &self.schemas {
            if reference.package == expected.package && reference.revision == expected.revision {
                return if reference == &expected {
                    Ok(())
                } else {
                    Err(SchemaError::ConflictingSchema)
                };
            }
        }
        descriptor.types.sort_by(|a, b| a.name.cmp(&b.name));
        descriptor.operations.sort_by(|a, b| a.name.cmp(&b.name));
        for ty in &mut descriptor.types {
            ty.constraints.sort();
            if let TypeShape::Variant { variants } = &mut ty.shape {
                variants.sort_by(|a, b| a.name.cmp(&b.name));
            }
        }
        self.finalized = false;
        self.schemas.push((expected, descriptor));
        Ok(())
    }
    fn fields_for<'a>(
        &'a self,
        value: &'a NdfValue,
    ) -> Result<(&'a [FieldDescriptor], &'a [NdfValue]), SchemaError> {
        let (schema, name) = match value {
            NdfValue::Record(v) => (&v.schema, &v.kind),
            NdfValue::Variant(v) => (&v.schema, &v.type_name),
            _ => return Err(SchemaError::WrongType),
        };
        let descriptor = self.descriptor(schema).ok_or(SchemaError::UnknownSchema)?;
        let definition = descriptor
            .types
            .iter()
            .find(|t| &t.name == name)
            .ok_or(SchemaError::UnknownType)?;
        match (&definition.shape, value) {
            (TypeShape::Record { fields }, NdfValue::Record(v)) => Ok((fields, &v.fields)),
            (TypeShape::Variant { variants }, NdfValue::Variant(v)) => Ok((
                &variants
                    .iter()
                    .find(|variant| variant.name == v.variant)
                    .ok_or(SchemaError::UnknownVariant)?
                    .fields,
                &v.fields,
            )),
            _ => Err(SchemaError::WrongType),
        }
    }

    /// Checks a standalone operation or reader type against the finalized selected schemas.
    pub fn validate_type(
        &self,
        ty: &TypeDescriptor,
        budget: &mut Budget,
    ) -> Result<(), SchemaError> {
        if !self.finalized {
            return Err(SchemaError::Unfinalized);
        }
        let mut ty = ty;
        let mut depth = 1u64;
        loop {
            budget.observe_depth(depth)?;
            budget.charge(Resource::Work, 1)?;
            match ty {
                TypeDescriptor::List(inner) | TypeDescriptor::Option(inner) => {
                    ty = inner;
                    depth = depth.checked_add(1).ok_or(StopReason::DepthLimit)?;
                }
                TypeDescriptor::Named(reference) => {
                    let schema = self
                        .selected(&reference.package, reference.revision)
                        .ok_or(SchemaError::UnknownSchema)?;
                    self.kind_id(schema, &reference.name)?;
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
    }
    /// Checks every symbolic reference, including unused variants and operation signatures.
    /// Registration may precede this pass so mutually referring schemas can be loaded together.
    pub fn finalize(&mut self, budget: &mut Budget) -> Result<(), SchemaError> {
        self.finalized = false;
        for (_, descriptor) in &self.schemas {
            let mut pending = Vec::new();
            let mut enqueue = |ty| -> Result<(), SchemaError> {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<&TypeDescriptor>() as u64,
                )?;
                pending.push(ty);
                Ok(())
            };
            for ty in &descriptor.types {
                match &ty.shape {
                    TypeShape::Record { fields } => {
                        for field in fields {
                            enqueue(&field.ty)?;
                        }
                    }
                    TypeShape::Variant { variants } => {
                        for variant in variants {
                            for field in &variant.fields {
                                enqueue(&field.ty)?;
                            }
                        }
                    }
                }
            }
            for op in &descriptor.operations {
                enqueue(&op.input)?;
                enqueue(&op.output)?;
            }
            while let Some(ty) = pending.pop() {
                budget.charge(Resource::Work, 1)?;
                match ty {
                    TypeDescriptor::List(inner) | TypeDescriptor::Option(inner) => {
                        pending.push(inner)
                    }
                    TypeDescriptor::Named(reference) => {
                        let (_, owner) = self
                            .schemas
                            .iter()
                            .find(|(r, _)| {
                                r.package == reference.package && r.revision == reference.revision
                            })
                            .ok_or(SchemaError::UnknownSchema)?;
                        if !owner.types.iter().any(|t| t.name == reference.name) {
                            return Err(SchemaError::UnknownType);
                        }
                    }
                    _ => {}
                }
            }
        }
        self.finalized = true;
        Ok(())
    }
    /// Local kind IDs are canonical scalar-sorted names, independent of descriptor insertion order.
    pub fn kind_id(&self, schema: &SchemaRef, name: &str) -> Result<u64, SchemaError> {
        self.descriptor(schema)
            .ok_or(SchemaError::UnknownSchema)?
            .types
            .iter()
            .position(|ty| ty.name == name)
            .map(|i| i as u64)
            .ok_or(SchemaError::UnknownType)
    }
    pub fn kind_name(&self, schema: &SchemaRef, id: u64) -> Result<&str, SchemaError> {
        let descriptor = self.descriptor(schema).ok_or(SchemaError::UnknownSchema)?;
        usize::try_from(id)
            .ok()
            .and_then(|i| descriptor.types.get(i))
            .map(|ty| ty.name.as_str())
            .ok_or(SchemaError::UnknownType)
    }
    pub fn descriptor(&self, reference: &SchemaRef) -> Option<&SchemaDescriptor> {
        self.schemas
            .iter()
            .find(|(r, _)| r == reference)
            .map(|(_, d)| d)
    }
    /// Iterative traversal avoids dependence on the machine call-stack for external values.
    pub fn validate<'a>(
        &self,
        expected: &TypeDescriptor,
        value: &'a NdfValue,
        budget: &mut Budget,
    ) -> Result<StructuralValue<'a>, SchemaError> {
        if !self.finalized {
            return Err(SchemaError::Unfinalized);
        }
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(&TypeDescriptor, &NdfValue, u64)>() as u64,
        )?;
        let mut pending = Vec::new();
        pending.push((expected, value, 1u64));
        while let Some((ty, value, depth)) = pending.pop() {
            budget.charge(Resource::Work, 1)?;
            budget.charge(Resource::Nodes, 1)?;
            budget.observe_depth(depth)?;
            let next_depth = depth.checked_add(1).ok_or(StopReason::DepthLimit)?;
            let mut push = |ty, value| -> Result<(), SchemaError> {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<(&TypeDescriptor, &NdfValue, u64)>() as u64,
                )?;
                pending.push((ty, value, next_depth));
                Ok(())
            };
            match (ty, value) {
                (
                    TypeDescriptor::NdfValue
                    | TypeDescriptor::NdfScalar
                    | TypeDescriptor::TypedValue,
                    value,
                ) => {
                    if matches!(ty, TypeDescriptor::NdfScalar)
                        && !matches!(
                            value,
                            NdfValue::Unit
                                | NdfValue::Bool(_)
                                | NdfValue::U64(_)
                                | NdfValue::Integer(_)
                                | NdfValue::Rational(_)
                                | NdfValue::Text(_)
                                | NdfValue::Bytes(_)
                        )
                    {
                        return Err(SchemaError::WrongType);
                    }
                    if matches!(ty, TypeDescriptor::TypedValue)
                        && !matches!(value, NdfValue::Record(_) | NdfValue::Variant(_))
                    {
                        return Err(SchemaError::WrongType);
                    }
                    match value {
                        NdfValue::Some(value) => push(ty, value)?,
                        NdfValue::List(values) => {
                            for value in values.iter().rev() {
                                push(ty, value)?;
                            }
                        }
                        NdfValue::Record(_) | NdfValue::Variant(_) => {
                            let (fields, values) = self.fields_for(value)?;
                            if fields.len() != values.len() {
                                return Err(SchemaError::FieldCount);
                            }
                            for (field, value) in fields.iter().zip(values).rev() {
                                push(&field.ty, value)?;
                            }
                        }
                        _ => {}
                    }
                }
                (TypeDescriptor::Unit, NdfValue::Unit)
                | (TypeDescriptor::Bool, NdfValue::Bool(_))
                | (TypeDescriptor::U64, NdfValue::U64(_))
                | (TypeDescriptor::Integer, NdfValue::Integer(_))
                | (TypeDescriptor::Rational, NdfValue::Rational(_))
                | (TypeDescriptor::Text, NdfValue::Text(_))
                | (TypeDescriptor::Bytes, NdfValue::Bytes(_)) => {}
                (TypeDescriptor::Natural, NdfValue::Integer(number)) if !number.is_negative() => {}
                (TypeDescriptor::Bytes32, NdfValue::Bytes(bytes)) if bytes.len() == 32 => {}
                (TypeDescriptor::Option(_), NdfValue::None) => {}
                (TypeDescriptor::Option(inner), NdfValue::Some(value)) => push(inner, value)?,
                (TypeDescriptor::List(inner), NdfValue::List(values)) => {
                    for value in values.iter().rev() {
                        push(inner, value)?;
                    }
                }
                (TypeDescriptor::Named(name), value) => {
                    let (reference, descriptor) = self
                        .schemas
                        .iter()
                        .find(|(r, _)| r.package == name.package && r.revision == name.revision)
                        .ok_or(SchemaError::UnknownSchema)?;
                    let definition = descriptor
                        .types
                        .iter()
                        .find(|t| t.name == name.name)
                        .ok_or(SchemaError::UnknownType)?;
                    let (fields, values) = match (&definition.shape, value) {
                        (TypeShape::Record { fields }, NdfValue::Record(record))
                            if &record.schema == reference && record.kind == name.name =>
                        {
                            (fields, &record.fields)
                        }
                        (TypeShape::Variant { variants }, NdfValue::Variant(variant))
                            if &variant.schema == reference && variant.type_name == name.name =>
                        {
                            (
                                &variants
                                    .iter()
                                    .find(|v| v.name == variant.variant)
                                    .ok_or(SchemaError::UnknownVariant)?
                                    .fields,
                                &variant.fields,
                            )
                        }
                        _ => return Err(SchemaError::WrongType),
                    };
                    if fields.len() != values.len() {
                        return Err(SchemaError::FieldCount);
                    }
                    for (field, value) in fields.iter().zip(values).rev() {
                        push(&field.ty, value)?;
                    }
                }
                _ => return Err(SchemaError::WrongType),
            }
        }
        Ok(StructuralValue { value })
    }
}
