//! Bounded host CLI for validating a first-seed JSON document on the main thread.
use nepl3_core::{
    budget::{Budget, Limits},
    source::SourceAdmission,
};
use std::io::{self, Read};
const MAX_JSON_BYTES: u64 = 16 * 1024 * 1024;
/// Read first-seed interchange from stdin, execute the typed importer, and print
/// its actual node count and usage. This command does not certify bootstrap.
pub fn seed_check() -> crate::Result<()> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take(MAX_JSON_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err("seed JSON exceeds 16 MiB".into());
    }
    let mut budget = Budget::new(Limits {
        source_bytes: 1024 * 1024,
        work: 1_000_000_000,
        depth: 512,
        nodes: 1_000_000,
        allocation_units: 4_000_000_000,
        output_bytes: 1024 * 1024,
        diagnostics: 1000,
        events: 1000,
    });
    let document = super::load(&bytes, &mut budget, &mut SourceAdmission::default())
        .map_err(|e| format!("seed import: {e:?}; usage: {:?}", budget.usage()))?;
    println!(
        "seed imported: {} nodes; usage: {:?}",
        document.nodes.len(),
        budget.usage()
    );
    Ok(())
}
