//! Concept maps through the command layer.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

fn setup(dir: &TempDir) -> (Api, i64, i64, i64) {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    let parent = ok(&mut a, "code.add", json!({ "name": "Barriers" }))["id"].as_i64().unwrap();
    let child = ok(&mut a, "code.add", json!({ "name": "Cost", "parent_id": parent }))["id"].as_i64().unwrap();
    let other = ok(&mut a, "code.add", json!({ "name": "Anxiety" }))["id"].as_i64().unwrap();
    (a, parent, child, other)
}

#[test]
fn a_map_is_made_filled_joined_arranged_and_changes_are_taken_back() {
    let dir = TempDir::new().unwrap();
    let (mut a, parent, child, other) = setup(&dir);
    let map = ok(&mut a, "map.create", json!({ "name": "Barriers" }));
    let id = map["id"].as_i64().unwrap();
    assert_eq!(map["nodes"], json!([]));
    assert_eq!(err(&mut a, "map.create", json!({ "name": "barriers" })).code, "invalid");

    let n1 = ok(&mut a, "map.node_add", json!({ "map_id": id, "kind": "code", "ref_id": child, "x": 0, "y": 0 }));
    assert_eq!((n1["label"].as_str(), n1["kind"].as_str()), (Some("Cost"), Some("code")));
    let n2 = ok(&mut a, "map.node_add", json!({ "map_id": id, "kind": "note", "label": "Why people stay away", "x": 220, "y": 0 }));
    assert_eq!(err(&mut a, "map.node_add", json!({ "map_id": id, "kind": "code", "ref_id": child })).code, "invalid", "a code is on a map once");
    assert_eq!(err(&mut a, "map.node_add", json!({ "map_id": id, "kind": "code", "ref_id": 9999 })).code, "not_found");
    assert_eq!(err(&mut a, "map.node_add", json!({ "map_id": id, "kind": "gadget" })).code, "invalid");

    let link = ok(&mut a, "map.link_add", json!({ "map_id": id, "from": n1["id"], "to": n2["id"], "label": "leads to" }));
    assert_eq!((link["label"].as_str(), link["directed"].as_bool()), (Some("leads to"), Some(true)), "a link has a direction unless told otherwise");
    assert_eq!(err(&mut a, "map.link_add", json!({ "map_id": id, "from": n1["id"], "to": n2["id"], "label": "leads to" })).code, "invalid");
    ok(&mut a, "map.link_update", json!({ "id": link["id"], "label": "causes", "directed": false }));
    ok(&mut a, "map.node_move", json!({ "id": n2["id"], "x": 300, "y": 40 }));
    ok(&mut a, "map.node_note", json!({ "id": n2["id"], "label": "Why people stay away from care" }));

    let added = ok(&mut a, "map.add_codes", json!({ "map_id": id, "code_ids": [parent, child, other], "hierarchy": true }));
    assert_eq!((added["nodes_added"].as_i64(), added["links_added"].as_i64(), added["already_there"].as_i64()), (Some(2), Some(1), Some(1)), "Cost was already there; Barriers is joined to it");
    let laid = ok(&mut a, "map.layout", json!({ "map_id": id, "how": "layers" }));
    assert_eq!(laid["nodes"].as_array().unwrap().len(), 4);
    assert_eq!(err(&mut a, "map.layout", json!({ "map_id": id, "how": "spiral" })).code, "invalid");

    let listed = ok(&mut a, "map.list", json!({}));
    assert_eq!((listed[0]["nodes"].as_i64(), listed[0]["links"].as_i64()), (Some(4), Some(2)));
    // A code renamed in the codebook is renamed on the map.
    ok(&mut a, "code.update", json!({ "id": child, "name": "Money" }));
    let got = ok(&mut a, "map.get", json!({ "id": id }));
    assert!(got["nodes"].as_array().unwrap().iter().any(|n| n["label"] == "Money"));

    assert_eq!(ok(&mut a, "undo", json!({}))["action"], "code.update");
    assert_eq!(ok(&mut a, "undo", json!({}))["action"], "map.layout");
    assert_eq!(ok(&mut a, "undo", json!({}))["action"], "map.add_codes");
    ok(&mut a, "map.node_delete", json!({ "id": n1["id"] }));
    assert_eq!(ok(&mut a, "map.get", json!({ "id": id }))["links"], json!([]), "taking a node off takes its links");
    ok(&mut a, "map.delete", json!({ "id": id }));
    assert_eq!(err(&mut a, "map.get", json!({ "id": id })).code, "not_found");
    assert_eq!(ok(&mut a, "undo", json!({}))["action"], "map.delete");
    assert_eq!(ok(&mut a, "map.get", json!({ "id": id }))["nodes"].as_array().unwrap().len(), 1);
}
