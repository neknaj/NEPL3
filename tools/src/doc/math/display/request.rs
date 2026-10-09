//! Native input assembly only. Retains the exact prepared Math owner, but does
//! not certify execution, cumulative preparation usage or result admission.
use super::{PreparedDisplay, TexPreparation};
use nepl3_core::budget::{Budget, Resource, StopReason};
use serde::Serialize;
use std::io::{self, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderLimits {
    pub input_bytes: u64,
    pub output_bytes: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseLimits {
    pub input_bytes: u64,
    pub nodes: u64,
    pub depth: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub timeout_millis: u64,
    pub diagnostic_bytes: u64,
    pub reply_bytes: u64,
    pub module_bytes: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controls {
    pub limits: RenderLimits,
    pub parse_limits: ParseLimits,
    pub options: Options,
}
#[derive(Debug)]
pub enum Error {
    InvalidControls,
    Encoding,
    Stopped(StopReason),
}
impl From<StopReason> for Error {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}

/// Immutable assembly tied to one prepared input. Copying its bytes does not
/// create an issued invocation or authorize a corresponding renderer reply.
pub struct PreparedRequest<'a> {
    owner: &'a PreparedDisplay,
    controls: Controls,
    bytes: Vec<u8>,
}
impl PreparedRequest<'_> {
    /// Exact copied controls used to serialize this request. Mutating a returned
    /// copy cannot change the retained wire or its selected input.
    pub fn controls(&self) -> Controls {
        self.controls
    }
    pub fn owner(&self) -> &PreparedDisplay {
        self.owner
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderInput<'a> {
    tex: &'a str,
    display_mode: bool,
    output: &'static str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Wire<'a> {
    version: u8,
    request: RenderInput<'a>,
    limits: RenderLimits,
    parse_limits: ParseLimits,
    options: Options,
}
struct Writer<'a> {
    bytes: Vec<u8>,
    cap: usize,
    budget: &'a mut Budget,
}
impl Write for Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = (|| {
            self.budget.poll()?;
            if bytes.len() > self.cap - self.bytes.len() {
                return Err(self.budget.stop(StopReason::OutputLimit));
            }
            self.budget.charge(Resource::Work, bytes.len() as u64)?;
            self.budget
                .charge(Resource::OutputBytes, bytes.len() as u64)?;
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        })();
        result.map_err(|_| io::Error::from(io::ErrorKind::Other))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Encode exactly the retained TeX and Doc-selected mode. None preserves the
/// owner's MathML-only/unsupported reason in owner.tex(); no request was made.
/// The bounded native buffer is prepaid; encoded transport bytes consume Work
/// and OutputBytes. This budget does not measure Node or earlier preparation.
pub fn prepare<'a>(
    owner: &'a PreparedDisplay,
    c: Controls,
    cap: usize,
    b: &mut Budget,
) -> Result<Option<PreparedRequest<'a>>, Error> {
    b.poll()?;
    const SAFE: u64 = (1u64 << 53) - 1;
    if [
        c.limits.input_bytes,
        c.limits.output_bytes,
        c.parse_limits.input_bytes,
        c.parse_limits.nodes,
        c.parse_limits.depth,
        c.options.diagnostic_bytes,
        c.options.reply_bytes,
        c.options.module_bytes,
    ]
    .into_iter()
    .any(|n| n > SAFE)
        || c.options.timeout_millis == 0
        || c.options.timeout_millis > 2147483647
    {
        return Err(Error::InvalidControls);
    }
    let TexPreparation::Ready { tex, .. } = owner.tex() else {
        return Ok(None);
    };
    if cap > isize::MAX as usize {
        return Err(b.stop(StopReason::AllocationLimit).into());
    }
    // serde scans unescaped runs before its next writer call.
    b.charge(Resource::Work, tex.len() as u64)?;
    b.charge(Resource::AllocationUnits, cap as u64)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(cap)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    let mut writer = Writer {
        bytes,
        cap,
        budget: b,
    };
    let result = serde_json::to_writer(
        &mut writer,
        &Wire {
            version: 1,
            request: RenderInput {
                tex,
                display_mode: matches!(owner.display(), nepl3_markup::mathml::Display::Block),
                output: "html",
            },
            limits: c.limits,
            parse_limits: c.parse_limits,
            options: c.options,
        },
    );
    writer.budget.poll()?;
    result.map_err(|_| Error::Encoding)?;
    Ok(Some(PreparedRequest {
        owner,
        controls: c,
        bytes: writer.bytes,
    }))
}

pub(super) mod owned;
