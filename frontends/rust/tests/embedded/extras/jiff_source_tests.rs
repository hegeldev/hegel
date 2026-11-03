use super::*;

#[test]
fn the_bundled_database_reports_the_jiff_tzdb_release() {
    assert_eq!(
        database_version("TimeZoneDatabase(Bundled(available))"),
        jiff_tzdb::VERSION.unwrap()
    );
}

#[test]
fn a_database_without_a_release_reports_none() {
    assert_eq!(database_version("TimeZoneDatabase(unavailable)"), "");
    assert_eq!(
        database_version("TimeZoneDatabase(Concatenated(/system/usr/share/zoneinfo/tzdata))"),
        ""
    );
}

#[test]
fn a_zoneinfo_directory_reports_its_tzdata_zi_release() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("tzdata.zi"),
        "# version 2026d-rearguard\n# redo posix_only\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("+VERSION"), "2020a\n").unwrap();
    let description = format!("TimeZoneDatabase(ZoneInfo({}))", dir.path().display());
    assert_eq!(database_version(&description), "2026d-rearguard");
}

#[test]
fn a_zoneinfo_directory_falls_back_to_its_version_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("tzdata.zi"), "# not a version line\n").unwrap();
    std::fs::write(dir.path().join("+VERSION"), "2026d\n").unwrap();
    assert_eq!(zoneinfo_version(dir.path()), "2026d");
    std::fs::remove_file(dir.path().join("tzdata.zi")).unwrap();
    assert_eq!(zoneinfo_version(dir.path()), "2026d");
}

#[test]
fn a_zoneinfo_directory_without_a_release_reports_none() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(zoneinfo_version(dir.path()), "");
}

#[test]
fn a_name_jiff_lacks_loads_nothing() {
    let source = zone_source();
    assert_eq!(source.info("Mars/Olympus_Mons").unwrap(), None);
}
