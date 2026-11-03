#[test]
fn test_engine_version_is_the_pinned_engine_crate_version() {
    let manifest = include_str!("../Cargo.toml");
    let pinned = manifest
        .lines()
        .find(|line| line.starts_with("hegeltest-c"))
        .and_then(|line| line.split("version = \"=").nth(1))
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("no exact hegeltest-c pin in {manifest}"));
    assert_eq!(hegel::engine_version(), pinned);
}

#[test]
fn test_engine_version_has_three_numeric_components() {
    let version = hegel::engine_version();
    let parts: Vec<&str> = version.split('.').collect();
    assert_eq!(parts.len(), 3, "{version}");
    for part in parts {
        part.parse::<u64>()
            .unwrap_or_else(|e| panic!("{version}: {part} is not a number: {e}"));
    }
}
