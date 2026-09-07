//! Explicit foundation Report conversion. Decoding validates declared source
//! references and typed payloads; operation identity and remote Usage attribution
//! must additionally be checked by the receiving operation boundary.
use crate::{WireError, boundary::typed::Codec, boundary::*, source::*};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Usage},
    diagnostic::*,
    schema::{SchemaError, SchemaRegistry},
    source::{Digest, SourceAdmission, SourceStore, TextEdit},
    value::{NdfValue, SchemaRef},
};
impl Codec for Digest {
    fn value(&self, _: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        bytes(&self.0, b)
    }
    fn from(
        v: &NdfValue,
        _: &SchemaRef,
        _: &SourceStore,
        _: &mut Budget,
    ) -> Result<Self, WireError> {
        as_digest(v)
    }
}
impl Codec for Severity {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        variant(
            s,
            "Severity",
            match self {
                Self::Error => "Error",
                Self::Warning => "Warning",
                Self::Information => "Information",
                Self::Hint => "Hint",
            },
            [],
            b,
        )
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        _: &SourceStore,
        _: &mut Budget,
    ) -> Result<Self, WireError> {
        let (name, f) = variant_parts(v, s, "Severity")?;
        if !f.is_empty() {
            return Err(WireError::InvalidType);
        }
        Ok(match name {
            "Error" => Self::Error,
            "Warning" => Self::Warning,
            "Information" => Self::Information,
            "Hint" => Self::Hint,
            _ => return Err(WireError::InvalidType),
        })
    }
}
macro_rules! record_codec{($ty:ident,$name:literal,$count:literal,[$($field:ident:$index:literal),*])=>{impl Codec for $ty{fn value(&self,s:&SchemaRef,b:&mut Budget)->Result<NdfValue,WireError>{record(s,$name,[$(self.$field.value(s,b)?),*],b)}fn from(v:&NdfValue,s:&SchemaRef,store:&SourceStore,b:&mut Budget)->Result<Self,WireError>{let f=fields(v,s,$name,$count)?;Ok(Self{$($field:Codec::from(&f[$index],s,store,b)?),*})}}};}
record_codec!(Usage,"Usage",8,[source_bytes:0,work:1,depth:2,nodes:3,allocation_units:4,output_bytes:5,diagnostics:6,events:7]);
record_codec!(TraceOverflow,"TraceOverflow",1,[dropped:0]);
record_codec!(TextEdit,"TextEdit",3,[span:0,expected_digest:1,replacement:2]);
record_codec!(Related,"Related",3,[span:0,code:1,arguments:2]);
record_codec!(Fix,"Fix",2,[id:0,edits:1]);
record_codec!(Diagnostic,"Diagnostic",8,[schema:0,code:1,severity:2,stage:3,arguments:4,primary:5,related:6,fixes:7]);
record_codec!(Event,"Event",5,[schema:0,kind:1,operation_path:2,span:3,payload:4]);
record_codec!(Report,"Report",4,[diagnostics:0,events:1,trace_overflow:2,usage:3]);

pub(crate) fn report_value(
    value: &Report,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    value.validate(sources, &[], registry, b)?;
    value.value(schema, b)
}
pub(crate) fn report_from(
    value: &NdfValue,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    b: &mut Budget,
) -> Result<Report, WireError> {
    let result = <Report as Codec>::from(value, schema, sources, b)?;
    result.validate(sources, &[], registry, b)?;
    Ok(result)
}
fn admission(
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<(), WireError> {
    for source in sources.snapshots() {
        admission.admit_existing(source, b)?;
    }
    Ok(())
}
/// `sources` is the operation's explicit declaration table, never an ambient store.
pub fn encode_report(
    value: &Report,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admitted: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    admission(sources, admitted, b)?;
    let schema = registry
        .selected("nepl3.foundation", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    let value = report_value(value, schema, registry, sources, b)?;
    crate::encode_checked(&value, &expected("Report"), registry, b)
}
pub fn decode_report(
    bytes: &[u8],
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admitted: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Report, WireError> {
    admission(sources, admitted, b)?;
    let schema = registry
        .selected("nepl3.foundation", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    let value = crate::decode_checked(bytes, &expected("Report"), registry, b)?;
    report_from(value.value(), schema, registry, sources, b)
}
