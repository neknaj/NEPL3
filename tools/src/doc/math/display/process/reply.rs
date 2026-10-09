//! Closed internal reply decoding. This preserves a native execution association,
//! not qualified bootstrap identity, visual fidelity or Doc admission.
use super::{Completed, Config};
use crate::doc::math::{
    display::{TexPreparation, request::PreparedRequest},
    katex,
};
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_markup::katex::fragment::{Fragment, Node};
use serde::Serialize;
use serde_json::{Map, Value};
use std::io::{self, Write};
mod catalog;
#[derive(Debug)]
pub enum Error {
    Json,
    Shape,
    Counter,
    Limit,
    Source,
    Encoding,
    Visual(katex::Error),
    Stopped(StopReason),
}
impl From<StopReason> for Error {
    fn from(x: StopReason) -> Self {
        Self::Stopped(x)
    }
}
pub struct Cause {
    code: &'static str,
    module: Option<&'static str>,
}
impl Cause {
    pub fn code(&self) -> &str {
        self.code
    }
    pub fn module(&self) -> Option<&str> {
        self.module
    }
}
/// Producer data: no HTML/CSS safety, rendering fidelity or Doc proof.
pub struct Visual {
    html: String,
    fragment: Fragment,
}
impl Visual {
    pub fn html(&self) -> &str {
        &self.html
    }
    pub fn fragment(&self) -> &Fragment {
        &self.fragment
    }
}
pub enum Outcome {
    InvalidRequest(Option<Cause>),
    Unavailable(Option<Cause>),
    RenderError,
    Stopped(Cause),
    Violation(Cause),
    Visual(Visual),
}
pub struct Diagnostic {
    pub level: Level,
    pub text: String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Level {
    Log,
    Info,
    Warn,
    Error,
    Debug,
}
pub struct Implementation {
    pub node: String,
    pub platform: String,
    pub arch: String,
    pub expected_vm_warnings: u64,
}
pub struct Observations {
    pub diagnostics: Vec<Diagnostic>,
    pub diagnostic_bytes: u64,
    pub implementation: Implementation,
    pub reply_bytes: u64,
}
pub struct Reply<'a> {
    request: &'a PreparedRequest<'a>,
    config: Config<'a>,
    outcome: Outcome,
    observations: Option<Observations>,
    termination_failure: bool,
}
impl Reply<'_> {
    // Only the owning display completion scope may detach validated reply data.
    pub(in crate::doc::math::display) fn into_owned_data(
        self,
    ) -> (Outcome, Option<Observations>, bool) {
        (self.outcome, self.observations, self.termination_failure)
    }

    pub fn request(&self) -> &PreparedRequest<'_> {
        self.request
    }
    pub fn config(&self) -> Config<'_> {
        self.config
    }
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }
    pub fn observations(&self) -> Option<&Observations> {
        self.observations.as_ref()
    }
    pub fn termination_failure(&self) -> bool {
        self.termination_failure
    }
}
type Fields = Map<String, Value>;
fn object(v: Value) -> Result<Fields, Error> {
    if let Value::Object(m) = v {
        Ok(m)
    } else {
        Err(Error::Shape)
    }
}
fn take(m: &mut Fields, k: &str, b: &mut Budget) -> Result<Value, Error> {
    b.charge(Resource::Work, 1)?;
    m.remove(k).ok_or(Error::Shape)
}
fn text(v: Value) -> Result<String, Error> {
    if let Value::String(s) = v {
        Ok(s)
    } else {
        Err(Error::Shape)
    }
}
fn natural(v: &Value) -> Result<u64, Error> {
    v.as_u64().filter(|n| *n < (1u64 << 53)).ok_or(Error::Shape)
}
fn number(m: &mut Fields, k: &str, b: &mut Budget) -> Result<u64, Error> {
    natural(&take(m, k, b)?)
}
fn empty(m: &Fields) -> Result<(), Error> {
    if m.is_empty() {
        Ok(())
    } else {
        Err(Error::Shape)
    }
}
fn reserve<T>(n: usize, b: &mut Budget) -> Result<Vec<T>, Error> {
    let bytes = n
        .checked_mul(core::mem::size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(v)
}
struct Counter<'a> {
    bytes: u64,
    budget: &'a mut Budget,
}
impl Write for Counter<'_> {
    fn write(&mut self, x: &[u8]) -> io::Result<usize> {
        self.budget
            .charge(Resource::Work, x.len() as u64)
            .map_err(|_| io::Error::from(io::ErrorKind::Other))?;
        self.bytes = self
            .bytes
            .checked_add(x.len() as u64)
            .ok_or_else(|| io::Error::from(io::ErrorKind::Other))?;
        Ok(x.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Inner<'a> {
    result: &'a Value,
    diagnostics: &'a Value,
    diagnostic_bytes: &'a Value,
    implementation: &'a Value,
}
fn inner_bytes(m: &Fields, b: &mut Budget) -> Result<u64, Error> {
    let get = |k: &str| m.get(k).ok_or(Error::Shape);
    let v = Inner {
        result: get("result")?,
        diagnostics: get("diagnostics")?,
        diagnostic_bytes: get("diagnosticBytes")?,
        implementation: get("implementation")?,
    };
    let mut w = Counter {
        bytes: 0,
        budget: b,
    };
    let result = serde_json::to_writer(&mut w, &v);
    w.budget.poll()?;
    result.map_err(|_| Error::Encoding)?;
    Ok(w.bytes)
}
/// Consumes the real completed transport. JSON storage is conservatively admitted
/// at 128 units per byte before duplicate-check and value decoding, following the
/// native bootstrap convention; this is not measured allocator or external usage.
pub fn decode<'a>(c: Completed<'a>, b: &mut Budget) -> Result<Reply<'a>, Error> {
    let result = decode_inner(c, b);
    b.poll()?;
    result
}
fn decode_inner<'a>(c: Completed<'a>, b: &mut Budget) -> Result<Reply<'a>, Error> {
    b.poll()?;
    let n = c.bytes.len() as u64;
    let allocation = n
        .checked_mul(128)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    let work = n
        .checked_mul(4)
        .ok_or_else(|| b.stop(StopReason::WorkLimit))?;
    b.charge(Resource::AllocationUnits, allocation)?;
    b.charge(Resource::Work, work)?;
    let s = std::str::from_utf8(&c.bytes).map_err(|_| Error::Json)?;
    crate::repository::json::validate(s).map_err(|_| Error::Json)?;
    let value = serde_json::from_str(s).map_err(|_| Error::Json)?;
    let mut m = object(value)?;
    if number(&mut m, "version", b)? != 1 {
        return Err(Error::Shape);
    }
    let termination_failure = match m.remove("terminationFailure") {
        None => false,
        Some(Value::Bool(true)) => true,
        _ => return Err(Error::Shape),
    };
    let keys = [
        "diagnostics",
        "diagnosticBytes",
        "implementation",
        "replyBytes",
    ];
    let count = keys.iter().filter(|k| m.contains_key(**k)).count();
    if count != 0 && count != keys.len() {
        return Err(Error::Shape);
    }
    let observed = count != 0;
    let controls = c.request.controls();
    let observations = if observed {
        let size = inner_bytes(&m, b)?;
        let reply_bytes = number(&mut m, "replyBytes", b)?;
        if size != reply_bytes {
            return Err(Error::Counter);
        }
        if size > controls.options.reply_bytes {
            return Err(Error::Limit);
        }
        let diagnostic_bytes = number(&mut m, "diagnosticBytes", b)?;
        if diagnostic_bytes > controls.options.diagnostic_bytes {
            return Err(Error::Limit);
        }
        let Value::Array(values) = take(&mut m, "diagnostics", b)? else {
            return Err(Error::Shape);
        };
        if values.len() as u64 > diagnostic_bytes {
            return Err(Error::Counter);
        }
        let mut diagnostics = reserve(values.len(), b)?;
        let mut sum = 0u64;
        for v in values {
            let mut d = object(v)?;
            let level = match text(take(&mut d, "level", b)?)?.as_str() {
                "log" => Level::Log,
                "info" => Level::Info,
                "warn" => Level::Warn,
                "error" => Level::Error,
                "debug" => Level::Debug,
                _ => return Err(Error::Shape),
            };
            let text = text(take(&mut d, "text", b)?)?;
            empty(&d)?;
            sum = sum
                .checked_add(text.len() as u64)
                .and_then(|n| n.checked_add(1))
                .ok_or(Error::Counter)?;
            diagnostics.push(Diagnostic { level, text });
        }
        if sum != diagnostic_bytes {
            return Err(Error::Counter);
        }
        let mut i = object(take(&mut m, "implementation", b)?)?;
        let implementation = Implementation {
            node: text(take(&mut i, "node", b)?)?,
            platform: text(take(&mut i, "platform", b)?)?,
            arch: text(take(&mut i, "arch", b)?)?,
            expected_vm_warnings: number(&mut i, "expectedVmWarnings", b)?,
        };
        if implementation.expected_vm_warnings > 1 {
            return Err(Error::Counter);
        }
        empty(&i)?;
        Some(Observations {
            diagnostics,
            diagnostic_bytes,
            implementation,
            reply_bytes,
        })
    } else {
        None
    };
    let result = object(take(&mut m, "result", b)?)?;
    empty(&m)?;
    let outcome = outcome(result, observed, termination_failure, c.request, b)?;
    Ok(Reply {
        request: c.request,
        config: c.config,
        outcome,
        observations,
        termination_failure,
    })
}
fn cause(m: &mut Fields, code: &'static str, module: bool, b: &mut Budget) -> Result<Cause, Error> {
    let module = if module {
        let id = text(take(m, "id", b)?)?;
        Some(
            *catalog::IDS
                .iter()
                .find(|s| **s == id)
                .ok_or(Error::Source)?,
        )
    } else {
        None
    };
    empty(m)?;
    Ok(Cause { code, module })
}
fn outcome(
    mut m: Fields,
    observed: bool,
    termination: bool,
    request: &PreparedRequest<'_>,
    b: &mut Budget,
) -> Result<Outcome, Error> {
    let kind = text(take(&mut m, "kind", b)?)?;
    if kind == "visual-parsed-unchecked" {
        if !observed {
            return Err(Error::Shape);
        }
        return visual(m, request, b).map(Outcome::Visual);
    }
    let reason = m.remove("reason").map(text).transpose()?;
    match kind.as_str() {
        "invalid-request" => {
            if !observed && termination {
                return Err(Error::Shape);
            }
            let c = match reason.as_deref() {
                None => {
                    empty(&m)?;
                    None
                }
                Some(r) => {
                    if observed || termination {
                        return Err(Error::Shape);
                    }
                    let code = match r {
                        "transport-config" => "transport-config",
                        "transport-json" => "transport-json",
                        "transport-canonical" => "transport-canonical",
                        "transport-schema" => "transport-schema",
                        _ => return Err(Error::Shape),
                    };
                    Some(cause(&mut m, code, false, b)?)
                }
            };
            Ok(Outcome::InvalidRequest(c))
        }
        "unavailable" => {
            if !observed {
                return Err(Error::Shape);
            }
            Ok(Outcome::Unavailable(match reason.as_deref() {
                None => {
                    empty(&m)?;
                    None
                }
                Some("vm-modules") => Some(cause(&mut m, "vm-modules", false, b)?),
                Some("module-file") => Some(cause(&mut m, "module-file", true, b)?),
                _ => return Err(Error::Shape),
            }))
        }
        "render-error" => {
            if !observed || reason.as_deref() != Some("parse") {
                return Err(Error::Shape);
            }
            empty(&m)?;
            Ok(Outcome::RenderError)
        }
        "stopped" => {
            let r = reason.as_deref().ok_or(Error::Shape)?;
            let (code, mode, no_termination) = match r {
                "transport-input-limit" => ("transport-input-limit", 0, true),
                "transport-output-limit" => ("transport-output-limit", 0, true),
                "transport-deadline" => ("transport-deadline", 0, true),
                "reply-limit" => ("reply-limit", 0, false),
                "deadline" => ("deadline", 0, false),
                "cancelled" => ("cancelled", 0, false),
                "input-limit" => ("input-limit", 2, !observed),
                "output-limit" => ("output-limit", 1, false),
                "expansion-limit" => ("expansion-limit", 1, false),
                "module-limit" => ("module-limit", 1, false),
                "node-limit" => ("node-limit", 1, false),
                "depth-limit" => ("depth-limit", 1, false),
                "diagnostic-limit" => ("diagnostic-limit", 1, false),
                _ => return Err(Error::Shape),
            };
            if (mode == 0 && observed)
                || (mode == 1 && !observed)
                || (no_termination && termination)
            {
                return Err(Error::Shape);
            }
            Ok(Outcome::Stopped(cause(&mut m, code, false, b)?))
        }
        "provider-violation" => {
            let r = reason.as_deref().ok_or(Error::Shape)?;
            let (code, obs, module, no_termination) = match r {
                "transport-input" => ("transport-input", false, false, true),
                "transport-exception" => ("transport-exception", false, false, true),
                "worker-start" => ("worker-start", false, false, true),
                "worker-exit" => ("worker-exit", false, false, true),
                "worker" => ("worker", false, false, false),
                "message" => ("message", false, false, false),
                "stdio" => ("stdio", false, false, false),
                "serialization" => ("serialization", false, false, false),
                "renderer" => ("renderer", true, false, false),
                "output" => ("output", true, false, false),
                "exception" => ("exception", true, false, false),
                "module-read" => ("module-read", true, false, false),
                "unissued-module-bundle" => ("unissued-module-bundle", true, false, false),
                "module-graph" => ("module-graph", true, false, false),
                "module-execution" => ("module-execution", true, false, false),
                "parser-export" => ("parser-export", true, false, false),
                "markup" => ("markup", true, false, false),
                "parser" => ("parser", true, false, false),
                "runtime-warning" => ("runtime-warning", true, false, false),
                "module-size" => ("module-size", true, true, false),
                "module-identity" => ("module-identity", true, true, false),
                "module-encoding" => ("module-encoding", true, true, false),
                _ => return Err(Error::Shape),
            };
            if observed != obs || (no_termination && termination) {
                return Err(Error::Shape);
            }
            Ok(Outcome::Violation(cause(&mut m, code, module, b)?))
        }
        _ => Err(Error::Shape),
    }
}
fn visual(mut m: Fields, request: &PreparedRequest<'_>, b: &mut Budget) -> Result<Visual, Error> {
    if text(take(&mut m, "version", b)?)? != catalog::VERSION
        || text(take(&mut m, "sourceIdentity", b)?)? != catalog::IDENTITY
        || text(take(&mut m, "loaderPolicy", b)?)? != catalog::LOADER
    {
        return Err(Error::Source);
    }
    if number(&mut m, "sourceBytes", b)? != catalog::BYTES
        || number(&mut m, "moduleCount", b)? != catalog::IDS.len() as u64
    {
        return Err(Error::Source);
    }
    let c = request.controls();
    if catalog::BYTES > c.options.module_bytes {
        return Err(Error::Limit);
    }
    let TexPreparation::Ready { tex, .. } = request.owner().tex() else {
        return Err(Error::Source);
    };
    let input = number(&mut m, "inputBytes", b)?;
    let output = number(&mut m, "outputBytes", b)?;
    let html = text(take(&mut m, "html", b)?)?;
    if input != tex.len() as u64 || output != html.len() as u64 {
        return Err(Error::Counter);
    }
    if input > c.limits.input_bytes
        || output > c.limits.output_bytes
        || output > c.parse_limits.input_bytes
    {
        return Err(Error::Limit);
    }
    let fragment = katex::parse(take(&mut m, "visual", b)?, b).map_err(|e| match e {
        katex::Error::Stopped(s) => Error::Stopped(s),
        e => Error::Visual(e),
    })?;
    empty(&m)?;
    let n = fragment.nodes.len();
    if n == 0 {
        return Err(Error::Shape);
    }
    if n as u64 > c.parse_limits.nodes {
        return Err(Error::Limit);
    }
    let mut depths = reserve(n, b)?;
    let mut parents = reserve(n, b)?;
    b.charge(Resource::Work, n as u64)?;
    parents.resize(n, false);
    for (i, node) in fragment.nodes.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let children: &[u64] = match node {
            Node::Span { children, .. } | Node::Svg { children, .. } => children,
            _ => &[],
        };
        let mut depth = 1u64;
        for child in children {
            b.charge(Resource::Work, 1)?;
            let j = usize::try_from(*child).map_err(|_| Error::Shape)?;
            if j >= i || parents[j] {
                return Err(Error::Shape);
            }
            parents[j] = true;
            depth = depth.max(depths[j] + 1);
        }
        if depth > c.parse_limits.depth {
            return Err(Error::Limit);
        }
        depths.push(depth);
    }
    if !matches!(fragment.nodes.last(), Some(Node::Span { .. }))
        || parents[..n - 1].iter().any(|p| !*p)
        || parents[n - 1]
    {
        return Err(Error::Shape);
    }
    Ok(Visual { html, fragment })
}

pub mod visual;
