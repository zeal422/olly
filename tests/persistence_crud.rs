mod common;

#[test]
fn persistence_crud_placeholder() {
    let db = common::temp_db::TempDb::new("target/test/workspaces.db");
    assert!(db.path.ends_with(".db"));
}
