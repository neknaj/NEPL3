//! Explicit host authority, not executable/bridge provenance or sandboxing.
use super::*;
use serde::Deserialize;
use std::path::PathBuf;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Host {
    pub node: PathBuf,
    pub bridge: PathBuf,
    pub modules_url: String,
    pub katex_installation: PathBuf,
}
impl Host {
    pub fn read(path: &Path) -> Result<(Self, Digest), String> {
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(err)?
            .take(16385)
            .read_to_end(&mut bytes)
            .map_err(err)?;
        if bytes.len() > 16384 {
            return Err("NativeHostConfigLimit".into());
        }
        let host: Self = serde_json::from_slice(&bytes).map_err(err)?;
        if !host.node.is_absolute()
            || !host.bridge.is_absolute()
            || !host.katex_installation.is_absolute()
        {
            return Err("NativeHostPathsMustBeAbsolute: PATH search and relative selections are not supported".into());
        }
        // A local absolute directory URL with no authority/query/fragment. URL
        // semantics and percent decoding remain checked by the selected bridge.
        if !host.modules_url.starts_with("file:///")
            || !host.modules_url.ends_with('/')
            || host.modules_url.contains(['?', '#', '\\'])
            || host.modules_url.bytes().any(|c| c <= 32 || c >= 127)
        {
            return Err("NativeHostModulesMustBeLocalDirectoryUrl".into());
        }
        Ok((host, Digest::of(&bytes)))
    }
    pub fn config(&self) -> process::Config<'_> {
        process::Config {
            node: &self.node,
            bridge: &self.bridge,
            modules_url: &self.modules_url,
            input_cap: 65536,
            output_cap: 1_000_000,
            timeout: Duration::from_secs(10),
        }
    }
}
pub(super) fn controls() -> request::Controls {
    request::Controls {
        limits: request::RenderLimits {
            input_bytes: 10000,
            output_bytes: 100000,
        },
        parse_limits: request::ParseLimits {
            input_bytes: 100000,
            nodes: 10000,
            depth: 100,
        },
        options: request::Options {
            timeout_millis: 5000,
            diagnostic_bytes: 10000,
            reply_bytes: 1_000_000,
            module_bytes: 953744,
        },
    }
}
