#[test]
fn crate_version_matches_manifest() {
    assert_eq!(typeul::VERSION, env!("CARGO_PKG_VERSION"));
}
