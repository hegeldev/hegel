use super::*;

#[test]
#[should_panic(expected = "jiff's time zone database is empty")]
fn an_empty_database_is_a_usage_error() {
    require_a_database(true);
}

#[test]
fn a_database_with_names_is_drawable() {
    require_a_database(false);
}
