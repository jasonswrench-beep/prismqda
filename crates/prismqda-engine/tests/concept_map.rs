//! Concept maps: maps with nodes and links, putting codes on a map with the codebook's hierarchy and how often codes occur
//! together, layouts, and taking every kind of change back.

use prismqda_engine::{ConceptMap, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project, SourceType};
use tempfile::TempDir;

struct W {
    p: Project,
    _dir: TempDir,
    parent: i64,
    child: i64,
    other: i64,
}

fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let src = p.add_source(&NewSource::new("Doc", SourceType::Document, "Fares cost too much and delays make me anxious. Parks are nice.")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let parent = p.add_code(&NewCode::new("Barriers")).unwrap();
    let mut c = NewCode::new("Cost");
    c.parent_id = Some(parent);
    let child = p.add_code(&c).unwrap();
    let other = p.add_code(&NewCode::new("Anxiety")).unwrap();
    // Cost and Anxiety on the same words, twice (two documents' worth of overlap would be more; one passage twice is enough).
    p.apply_coding(&NewCoding::new(src, 0, 20, child, ada)).unwrap();
    p.apply_coding(&NewCoding::new(src, 0, 20, other, ada)).unwrap();
    p.apply_coding(&NewCoding::new(src, 30, 40, child, ada)).unwrap();
    p.apply_coding(&NewCoding::new(src, 30, 40, other, ada)).unwrap();
    W { p, _dir: dir, parent, child, other }
}

fn names(m: &ConceptMap) -> Vec<String> {
    m.nodes.iter().map(|n| n.label.clone()).collect()
}

#[test]
fn maps_are_made_renamed_listed_and_deleted_under_names_that_are_used_once() {
    let mut w = world();
    let a = w.p.create_concept_map("Barriers").unwrap();
    let b = w.p.create_concept_map("A first sketch").unwrap();
    assert_eq!(w.p.list_concept_maps().unwrap().iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["A first sketch", "Barriers"], "in the order of their names");
    assert!(w.p.create_concept_map("BARRIERS").unwrap_err().to_string().contains("already a map named"));
    assert!(w.p.create_concept_map("  ").is_err());
    assert!(w.p.create_concept_map(&"x".repeat(81)).is_err());
    let m = w.p.update_concept_map(a, Some("Barriers to care"), Some("  first pass  ")).unwrap();
    assert_eq!((m.name.as_str(), m.notes.as_str()), ("Barriers to care", "first pass"));
    assert!(w.p.update_concept_map(a, Some("a first sketch"), None).is_err());
    w.p.delete_concept_map(b).unwrap();
    assert!(w.p.get_concept_map(b).is_err());
    assert_eq!(w.p.list_concept_maps().unwrap().len(), 1);
}

#[test]
fn nodes_stand_for_codes_cases_memos_or_notes_and_take_their_words_from_what_they_stand_for() {
    let mut w = world();
    let map = w.p.create_concept_map("M").unwrap();
    let n = w.p.add_map_node(map, "code", Some(w.child), "", 10.0, 20.0).unwrap();
    assert_eq!((n.label.as_str(), n.kind.as_str(), n.x, n.y, n.missing), ("Cost", "code", 10.0, 20.0, false));
    assert!(w.p.add_map_node(map, "code", Some(w.child), "", 0.0, 0.0).unwrap_err().to_string().contains("already on this map"));
    assert!(w.p.add_map_node(map, "code", Some(99_999), "", 0.0, 0.0).is_err(), "a code that does not exist");
    assert!(w.p.add_map_node(map, "code", None, "", 0.0, 0.0).is_err());
    assert!(w.p.add_map_node(map, "note", None, "   ", 0.0, 0.0).is_err(), "a note needs words");
    assert!(w.p.add_map_node(map, "gadget", None, "x", 0.0, 0.0).is_err());
    assert!(w.p.add_map_node(map, "note", None, "x", f64::NAN, 0.0).is_err(), "a place is a number");
    let note = w.p.add_map_node(map, "note", None, "Why people stay away", 0.0, 100.0).unwrap();
    // Rename the code: the node follows it.
    w.p.update_code(w.child, &prismqda_engine::CodeEdit { name: Some("Money".to_string()), ..Default::default() }).unwrap();
    assert_eq!(names(&w.p.get_concept_map(map).unwrap()), ["Money", "Why people stay away"]);
    // A note's words can change; a code's node has no words of its own.
    w.p.update_map_note(note.id, "Why people stay away from care").unwrap();
    assert!(w.p.update_map_note(n.id, "x").unwrap_err().to_string().contains("only a note"));
    assert!(w.p.update_map_note(note.id, " ").is_err());
}

#[test]
fn links_join_two_nodes_of_one_map_with_words_and_are_never_repeated() {
    let mut w = world();
    let map = w.p.create_concept_map("M").unwrap();
    let other_map = w.p.create_concept_map("N").unwrap();
    let a = w.p.add_map_node(map, "code", Some(w.child), "", 0.0, 0.0).unwrap().id;
    let b = w.p.add_map_node(map, "code", Some(w.other), "", 200.0, 0.0).unwrap().id;
    let far = w.p.add_map_node(other_map, "code", Some(w.child), "", 0.0, 0.0).unwrap().id;
    let l = w.p.add_map_link(map, a, b, "leads to", true).unwrap();
    assert_eq!((l.from_node, l.to_node, l.label.as_str(), l.directed), (a, b, "leads to", true));
    assert!(w.p.add_map_link(map, a, a, "x", true).is_err(), "a link joins two different nodes");
    assert!(w.p.add_map_link(map, a, far, "x", true).unwrap_err().to_string().contains("same map"));
    assert!(w.p.add_map_link(map, a, b, "leads to", true).unwrap_err().to_string().contains("already joined"));
    assert!(w.p.add_map_link(map, a, b, "is part of", true).is_ok(), "another relation between the same two is fine");
    // An undirected link is the same either way round.
    w.p.add_map_link(map, a, b, "occur together", false).unwrap();
    assert!(w.p.add_map_link(map, b, a, "occur together", false).is_err());
    let m = w.p.get_concept_map(map).unwrap();
    assert_eq!(m.links.len(), 3);
    let changed = w.p.update_map_link(l.id, Some("causes"), Some(false)).unwrap();
    assert_eq!((changed.label.as_str(), changed.directed), ("causes", false));
    w.p.delete_map_link(l.id).unwrap();
    assert_eq!(w.p.get_concept_map(map).unwrap().links.len(), 2);
    assert!(w.p.delete_map_link(l.id).is_err());
}

#[test]
fn codes_can_be_added_in_one_step_with_the_codebooks_hierarchy_and_how_often_they_occur_together() {
    let mut w = world();
    let map = w.p.create_concept_map("M").unwrap();
    let added = w.p.add_codes_to_map(map, &[w.parent, w.child, w.other], true, Some(2)).unwrap();
    assert_eq!((added.nodes_added, added.links_added, added.already_there), (3, 2, 0), "a link for the parent and child, and one for the pair that overlap twice");
    let m = w.p.get_concept_map(map).unwrap();
    assert_eq!(m.nodes.len(), 3);
    let by_label = |label: &str| m.nodes.iter().find(|n| n.label == label).unwrap().id;
    let contains = m.links.iter().find(|l| l.label == "contains").unwrap();
    assert_eq!((contains.from_node, contains.to_node, contains.directed), (by_label("Barriers"), by_label("Cost"), true));
    let together = m.links.iter().find(|l| l.label.starts_with("occur together")).unwrap();
    assert_eq!((together.label.as_str(), together.directed), ("occur together 2 times", false));
    assert!((m.nodes[0].x, m.nodes[0].y) != (m.nodes[1].x, m.nodes[1].y), "each node has its own place");

    // Adding again adds nothing, and says so; a threshold above the count adds no link.
    let again = w.p.add_codes_to_map(map, &[w.parent, w.child], true, Some(2)).unwrap();
    assert_eq!((again.nodes_added, again.links_added, again.already_there), (0, 0, 2));
    let map2 = w.p.create_concept_map("M2").unwrap();
    let strict = w.p.add_codes_to_map(map2, &[w.child, w.other], false, Some(3)).unwrap();
    assert_eq!((strict.nodes_added, strict.links_added), (2, 0));
    assert!(w.p.add_codes_to_map(map2, &[], true, None).is_err());
    assert!(w.p.add_codes_to_map(map2, &[12_345], true, None).is_err());
}

#[test]
fn a_layout_places_every_node_and_one_undo_puts_them_all_back() {
    let mut w = world();
    let map = w.p.create_concept_map("M").unwrap();
    w.p.add_codes_to_map(map, &[w.parent, w.child, w.other], true, None).unwrap();
    let before: Vec<(f64, f64)> = w.p.get_concept_map(map).unwrap().nodes.iter().map(|n| (n.x, n.y)).collect();
    w.p.layout_concept_map(map, "layers").unwrap();
    let m = w.p.get_concept_map(map).unwrap();
    let y = |label: &str| m.nodes.iter().find(|n| n.label == label).unwrap().y;
    assert!(y("Barriers") < y("Cost"), "the parent is above the child it contains");
    assert!(w.p.layout_concept_map(map, "spiral").is_err());
    w.p.layout_concept_map(map, "grid").unwrap();
    w.p.layout_concept_map(map, "circle").unwrap();
    assert_eq!(w.p.next_undo().unwrap().as_deref(), Some("Moved every node back to where it was"));
    for _ in 0..3 {
        w.p.undo().unwrap().unwrap();
    }
    let back: Vec<(f64, f64)> = w.p.get_concept_map(map).unwrap().nodes.iter().map(|n| (n.x, n.y)).collect();
    assert_eq!(back, before, "three layouts, three undos, and every node is where it was");
}

#[test]
fn every_kind_of_change_to_a_map_can_be_taken_back_in_turn() {
    let mut w = world();
    let map = w.p.create_concept_map("M").unwrap();
    let a = w.p.add_map_node(map, "code", Some(w.child), "", 0.0, 0.0).unwrap().id;
    let b = w.p.add_map_node(map, "note", None, "a note", 100.0, 0.0).unwrap().id;
    let link = w.p.add_map_link(map, a, b, "relates", true).unwrap().id;
    w.p.update_map_link(link, Some("explains"), None).unwrap();
    w.p.move_map_node(a, 5.0, 6.0).unwrap();
    w.p.update_map_note(b, "the same note, reworded").unwrap();
    w.p.update_concept_map(map, Some("Renamed"), None).unwrap();
    w.p.delete_map_node(a).unwrap();
    assert_eq!(w.p.get_concept_map(map).unwrap().links.len(), 0, "taking a node off takes its links with it");
    w.p.delete_concept_map(map).unwrap();
    assert!(w.p.list_concept_maps().unwrap().is_empty());

    // The map comes back with its note; then the node with its link; then each change in turn.
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.delete");
    let m = w.p.get_concept_map(map).unwrap();
    assert_eq!((m.name.as_str(), m.nodes.len()), ("Renamed", 1));
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.node.delete");
    let m = w.p.get_concept_map(map).unwrap();
    assert_eq!((m.nodes.len(), m.links.len(), m.links[0].label.as_str()), (2, 1, "explains"), "the node is back with its link");
    assert_eq!((m.nodes[0].x, m.nodes[0].y), (5.0, 6.0));
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.update");
    assert_eq!(w.p.get_concept_map(map).unwrap().name, "M");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.node.update");
    assert_eq!(w.p.get_concept_map(map).unwrap().nodes[1].label, "a note");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.node.move");
    assert_eq!(w.p.get_concept_map(map).unwrap().nodes[0].x, 0.0);
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.link.update");
    assert_eq!(w.p.get_concept_map(map).unwrap().links[0].label, "relates");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.link.add");
    assert!(w.p.get_concept_map(map).unwrap().links.is_empty());
    w.p.delete_map_link(link).unwrap_err();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.node.add");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.node.add");
    assert!(w.p.get_concept_map(map).unwrap().nodes.is_empty());
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.create");
    assert!(w.p.list_concept_maps().unwrap().is_empty());
}

#[test]
fn adding_codes_and_deleting_a_link_are_undone_too() {
    let mut w = world();
    let map = w.p.create_concept_map("M").unwrap();
    w.p.add_codes_to_map(map, &[w.parent, w.child, w.other], true, Some(1)).unwrap();
    let link = w.p.get_concept_map(map).unwrap().links[0].id;
    w.p.delete_map_link(link).unwrap();
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.link.delete");
    assert!(w.p.get_concept_map(map).unwrap().links.iter().any(|l| l.id == link), "the link is back with its own number");
    assert_eq!(w.p.undo().unwrap().unwrap().action, "map.add_codes");
    let m = w.p.get_concept_map(map).unwrap();
    assert!(m.nodes.is_empty() && m.links.is_empty(), "the codes and the links they brought are gone");
}
