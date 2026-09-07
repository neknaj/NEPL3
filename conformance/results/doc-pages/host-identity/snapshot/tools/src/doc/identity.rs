use nepl3_core::source::Digest;
pub fn host_identity() -> Digest {
    Digest::of(
        concat!(
            include_str!("source.rs"),
            include_str!("host.rs"),
            include_str!("reader.rs"),
            include_str!("../../../crates/foundation/reader/src/builtin/provider.rs")
        )
        .as_bytes(),
    )
}
