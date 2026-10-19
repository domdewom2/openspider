use os_store::readonly_sql;
use rusqlite::Connection;
use serde_json::Value;

// spec: AGENT-014
#[test]
fn schema_resource_documents_every_actual_table_view_and_column() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schema.ospider");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(include_str!(
        "../../../tests/fixtures/agent-m2-sql/schema.sql"
    ))
    .unwrap();
    let schema = readonly_sql::schema(&path).unwrap();
    let objects = schema["objects"].as_array().unwrap();
    assert_eq!(objects.len(), 2);
    for object in objects {
        let name = object["name"].as_str().unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_xinfo(?1)")
            .unwrap();
        let expected = stmt
            .query_map([name], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let actual = object["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert!(object["description"].as_str().is_some());
        assert!(object["columns"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["type"] != Value::Null));
    }
}
