use super::*;
use crate::package::identity::{operation, schema, sorted};
use core::cmp::Ordering;
use nepl3_core::schema::CanonicalWriter;

fn ordered<'a, T>(
    values: &'a [T],
    cmp: impl Fn(&T, &T) -> Ordering,
    cost: impl Fn(&T, &T) -> u64,
    budget: &mut Budget,
) -> Result<Vec<&'a T>, ProfileError> {
    budget.charge(
        Resource::AllocationUnits,
        (values.len() * core::mem::size_of::<&T>()) as u64,
    )?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let mut i = result.len();
        while i > 0 {
            budget.charge(Resource::Work, cost(value, result[i - 1]))?;
            if cmp(result[i - 1], value) != Ordering::Greater {
                break;
            }
            i -= 1;
        }
        budget.charge(Resource::Work, (result.len() - i) as u64 + 1)?;
        result.insert(i, value);
    }
    Ok(result)
}
fn separator(out: &mut CanonicalWriter<'_>, index: usize) -> Result<(), ProfileError> {
    if index != 0 {
        out.push(",")?;
    }
    Ok(())
}
fn digest_bytes(out: &mut CanonicalWriter<'_>, v: Digest) -> Result<(), ProfileError> {
    out.push("[")?;
    for (i, b) in v.0.iter().enumerate() {
        separator(out, i)?;
        out.number(u64::from(*b))?;
    }
    out.push("]")?;
    Ok(())
}
pub(super) fn digest(profile: &ParseProfile, budget: &mut Budget) -> Result<Digest, ProfileError> {
    let mut out = CanonicalWriter::new(budget);
    out.push("{\"allowlist\":[")?;
    for (i, op) in ordered(
        &profile.allowlist,
        |a, b| (&a.schema, &a.name).cmp(&(&b.schema, &b.name)),
        |a, b| {
            a.schema.package.len().min(b.schema.package.len()) as u64
                + a.name.len().min(b.name.len()) as u64
                + 33
        },
        out.budget(),
    )?
    .iter()
    .enumerate()
    {
        separator(&mut out, i)?;
        operation(&mut out, op)?;
    }
    out.push("],\"categoryModes\":[")?;
    for (i, v) in sorted(
        &profile.category_modes,
        |v| (&v.alias, &v.category),
        out.budget(),
    )?
    .iter()
    .enumerate()
    {
        separator(&mut out, i)?;
        out.push("[")?;
        out.quoted(&v.alias)?;
        out.push(",")?;
        out.quoted(&v.category)?;
        out.push(",")?;
        out.quoted(&v.mode)?;
        out.push("]")?;
    }
    out.push("],\"id\":")?;
    out.quoted(&profile.id)?;
    out.push(",\"languages\":[")?;
    for (i, v) in sorted(&profile.languages, |v| (&v.alias, ""), out.budget())?
        .iter()
        .enumerate()
    {
        separator(&mut out, i)?;
        out.push("[")?;
        out.quoted(&v.alias)?;
        out.push(",")?;
        schema(&mut out, &v.package.schema)?;
        out.push(",")?;
        digest_bytes(&mut out, v.package.semantic_digest)?;
        out.push(",")?;
        out.quoted(&v.default_category)?;
        out.push("]")?;
    }
    let l = profile.limits;
    out.push("],\"limits\":[")?;
    for (i, v) in [
        l.source_bytes,
        l.work,
        l.depth,
        l.nodes,
        l.allocation_units,
        l.output_bytes,
        l.diagnostics,
        l.events,
    ]
    .iter()
    .enumerate()
    {
        separator(&mut out, i)?;
        out.number(*v)?;
    }
    out.push("],\"providers\":[")?;
    for (i, v) in ordered(
        &profile.providers,
        |a, b| {
            (&a.operation.schema, &a.operation.name).cmp(&(&b.operation.schema, &b.operation.name))
        },
        |a, b| {
            a.operation
                .schema
                .package
                .len()
                .min(b.operation.schema.package.len()) as u64
                + a.operation.name.len().min(b.operation.name.len()) as u64
                + 33
        },
        out.budget(),
    )?
    .iter()
    .enumerate()
    {
        separator(&mut out, i)?;
        out.push("[")?;
        operation(&mut out, &v.operation)?;
        out.push(",")?;
        out.quoted(&v.provider)?;
        out.push(",")?;
        out.number(v.revision)?;
        out.push(",")?;
        digest_bytes(&mut out, v.implementation_digest)?;
        out.push("]")?;
    }
    out.push("],\"resources\":[")?;
    for (i, v) in sorted(&profile.resources, |v| (&v.id, ""), out.budget())?
        .iter()
        .enumerate()
    {
        separator(&mut out, i)?;
        out.push("[")?;
        out.quoted(&v.id)?;
        out.push(",")?;
        digest_bytes(&mut out, v.digest)?;
        out.push("]")?;
    }
    out.push("],\"schemas\":[")?;
    for (i, v) in ordered(
        &profile.schemas,
        |a, b| a.cmp(b),
        |a, b| a.package.len().min(b.package.len()) as u64 + 33,
        out.budget(),
    )?
    .iter()
    .enumerate()
    {
        separator(&mut out, i)?;
        schema(&mut out, v)?;
    }
    out.push("]}")?;
    let bytes = out.finish();
    budget.charge(Resource::Work, bytes.len() as u64)?;
    Ok(Digest::domain(b"NEPL3-PARSE-PROFILE-1\0", &bytes))
}
