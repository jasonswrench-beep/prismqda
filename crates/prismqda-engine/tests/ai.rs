//! AI through prompt packets: what a packet holds and leaves out, how a pasted-back answer is checked before anything of it
//! reaches the review queue, how the run is kept, and what the AI-use paragraph says.

use prismqda_engine::{CodeEdit, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, PacketSpec, Project, SourceType};
use tempfile::TempDir;

const DOC: &str = "Fares are too high for me.\n\nThe stop is far from my house.\n\nI love the new buses.";

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

fn spec() -> PacketSpec {
    serde_json::from_str("{}").unwrap()
}

struct World {
    p: Project,
    src: i64,
    ada: i64,
    cost: i64,
    access: i64,
}

fn world(dir: &TempDir) -> World {
    let mut p = project(dir);
    let src = p.add_source(&NewSource::new("Riders", SourceType::Document, DOC)).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let mut cost = NewCode::new("Cost");
    cost.definition = "Talk about money.".into();
    cost.inclusion_criteria = "prices or fares come up".into();
    let cost = p.add_code(&cost).unwrap();
    let access = p.add_code(&NewCode::new("Access")).unwrap();
    World { p, src, ada, cost, access }
}

fn start_of(needle: &str) -> i64 {
    DOC.find(needle).unwrap() as i64
}

#[test]
fn a_packet_holds_the_codebook_and_the_passages_and_leaves_out_what_may_not_be_sent() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let secret = w.p.add_code(&NewCode::new("Secret plan")).unwrap();
    w.p.update_code(secret, &CodeEdit { do_not_share: Some(true), ..Default::default() }).unwrap();
    let hidden = w.p.add_code(&NewCode::new("Hidden")).unwrap();
    w.p.update_code(hidden, &CodeEdit { ai_suggestible: Some(false), ..Default::default() }).unwrap();
    let kept_back = w.p.add_source(&NewSource::new("Kept back", SourceType::Document, "Private words that must stay here.")).unwrap();
    w.p.set_source_do_not_share(kept_back, true).unwrap();

    let packet = w.p.build_packet(&spec(), w.ada).unwrap();
    assert_eq!(packet.passages, 3);
    let mut names: Vec<&str> = packet.codes.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Access", "Cost"]);
    assert_eq!(packet.left_out.documents_not_shared, 1);
    let mut left_codes = packet.left_out.codes.clone();
    left_codes.sort();
    assert_eq!(left_codes, ["Hidden", "Secret plan"]);
    assert_eq!((packet.more_after, packet.next_skip), (0, 3));

    let text = &packet.prompt;
    assert!(text.contains("- Cost: Talk about money. Use it when: prices or fares come up.\n"), "{text}");
    assert!(text.contains("- Access\n"));
    assert!(text.contains("[P1] Fares are too high for me.\n[P2] The stop is far from my house.\n[P3] I love the new buses.\n"), "{text}");
    for never in ["Secret plan", "Hidden", "Private words"] {
        assert!(!text.contains(never), "{never} must not be in the packet");
    }
    assert_eq!(packet.characters, text.chars().count());

    // The run is kept at once, unanswered, with the exact text and its hash.
    let runs = w.p.ai_runs().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!((runs[0].id, runs[0].answered, runs[0].passages, runs[0].codes), (packet.run_id, false, 3, 2));
    assert_eq!(runs[0].prompt_hash, prismqda_engine::sha256_hex(text.as_bytes()));
    assert_eq!(w.p.ai_run(packet.run_id).unwrap().prompt, *text);
}

#[test]
fn passages_already_coded_by_this_coder_stay_out_unless_asked_for() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let s = start_of("Fares");
    w.p.apply_coding(&NewCoding::new(w.src, s, s + 5, w.cost, w.ada)).unwrap();
    let bo = w.p.add_coder(&NewCoder::human("Bo")).unwrap();
    let t = start_of("The stop");
    w.p.apply_coding(&NewCoding::new(w.src, t, t + 8, w.access, bo)).unwrap();

    let packet = w.p.build_packet(&spec(), w.ada).unwrap();
    assert_eq!(packet.passages, 2, "Ada coded the first passage; Bo's coding does not count for Ada");
    assert_eq!(packet.left_out.already_coded, 1);
    assert!(!packet.prompt.contains("Fares are too high"));

    let all = w.p.build_packet(&PacketSpec { only_uncoded: false, ..spec() }, w.ada).unwrap();
    assert_eq!(all.passages, 3);
    assert_eq!(all.left_out.already_coded, 0);
}

#[test]
fn a_long_list_is_sent_in_pieces_and_a_piece_can_be_asked_for_by_where_it_starts() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let first = w.p.build_packet(&PacketSpec { limit: Some(2), ..spec() }, w.ada).unwrap();
    assert_eq!((first.passages, first.more_after, first.next_skip), (2, 1, 2));
    assert!(first.prompt.contains("[P2] The stop") && !first.prompt.contains("I love the new buses"));

    let next = w.p.build_packet(&PacketSpec { limit: Some(2), skip: first.next_skip, ..spec() }, w.ada).unwrap();
    assert_eq!((next.passages, next.more_after), (1, 0));
    assert!(next.prompt.contains("[P1] I love the new buses."), "labels start again at P1 in each packet");

    let error = w.p.build_packet(&PacketSpec { skip: 3, ..spec() }, w.ada).unwrap_err().to_string();
    assert!(error.contains("nothing after"), "{error}");
    let error = w.p.build_packet(&PacketSpec { code_ids: vec![9999], ..spec() }, w.ada).unwrap_err().to_string();
    assert!(error.to_lowercase().contains("code"), "{error}");
}

#[test]
fn nothing_can_be_built_when_ai_is_off_and_the_setting_is_checked() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    assert_eq!(w.p.info().unwrap().ai_mode, "packet");
    w.p.set_ai_mode("off").unwrap();
    let error = w.p.build_packet(&spec(), w.ada).unwrap_err().to_string();
    assert!(error.contains("AI is off"), "{error}");
    assert!(w.p.set_ai_mode("cloud").is_err());
    w.p.set_ai_mode("packet").unwrap();
    assert!(w.p.build_packet(&spec(), w.ada).is_ok());
    // Both changes are in the log, and an undo puts the setting back.
    w.p.set_ai_mode("off").unwrap();
    w.p.undo().unwrap();
    assert_eq!(w.p.info().unwrap().ai_mode, "packet");
}

#[test]
fn an_answer_is_checked_and_only_what_passes_reaches_the_review_queue() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let packet = w.p.build_packet(&spec(), w.ada).unwrap();

    let answer = r#"Sure, here is my answer [as asked]:
```json
[
  {"passage": "P1", "code": "Cost", "quote": "too  high", "reason": "The rider talks about fares."},
  {"passage": "P2", "code": "access", "quote": "", "reason": "Distance to the stop."},
  {"passage": "P3", "code": "Cost", "quote": "invented words that are not there", "reason": "Made up."},
  {"passage": "P9", "code": "Cost", "quote": "x", "reason": "No such passage."},
  {"passage": "P1", "code": "Joy", "quote": "Fares", "reason": "No such code."},
  {"passage": "P1", "code": "Cost", "quote": "too high", "reason": "Same thing again."},
  "not an object"
]
```
Hope that helps!"#;
    let outcome = w.p.answer_packet(packet.run_id, "Test Chatbot 1.0", answer).unwrap();
    assert_eq!((outcome.read, outcome.suggested), (7, 2));
    let why: Vec<(usize, &str)> = outcome.rejected.iter().map(|r| (r.number, r.why.as_str())).collect();
    assert_eq!(why.len(), 5, "{why:?}");
    assert_eq!(why[0].0, 3);
    assert!(why[0].1.contains("not in passage P3, word for word"), "{why:?}");
    assert!(why[1].1.contains("no passage P9"), "{why:?}");
    assert!(why[2].1.contains("Joy"), "{why:?}");
    assert!(why[3].1.contains("already in the answer"), "{why:?}");
    assert!(why[4].1.contains("not an object"), "{why:?}");

    // What waits in the queue: the quoted words only for P1, the whole passage for P2, each tied to the run.
    let queue = w.p.list_proposals(Some("pending"), None, 50).unwrap();
    assert_eq!(queue.len(), 2);
    assert!(queue.iter().all(|q| q.origin == "ai" && q.ai_run_id == Some(packet.run_id) && q.coder_id == w.ada));
    let p1 = queue.iter().find(|q| q.code_id == w.cost).unwrap();
    assert_eq!(w.p.text_of_span(w.src, p1.start, p1.end).unwrap(), "too high");
    assert_eq!(p1.rationale, "AI suggestion (Test Chatbot 1.0): The rider talks about fares.");
    let p2 = queue.iter().find(|q| q.code_id == w.access).unwrap();
    assert_eq!(w.p.text_of_span(w.src, p2.start, p2.end).unwrap(), "The stop is far from my house.");

    // The run keeps the model, the raw answer and what the checker refused.
    let run = w.p.ai_run(packet.run_id).unwrap();
    assert_eq!((run.run.model.as_str(), run.run.answered, run.run.read, run.run.suggested), ("Test Chatbot 1.0", true, 7, 2));
    assert_eq!((run.run.checker_rejected, run.run.pending, run.run.accepted, run.run.rejected), (5, 2, 0, 0));
    assert_eq!(run.raw_output, answer);
    assert_eq!(run.checker_rejections.len(), 5);
    assert_eq!(run.run.batch, outcome.batch, "the run knows which queue batch holds its suggestions");
    assert!(outcome.batch.is_some());

    // Nothing is coded until a person decides; an accepted suggestion keeps where it came from.
    assert!(w.p.list_codings(w.src).unwrap().is_empty());
    w.p.decide_proposals(&[p1.id], true).unwrap();
    w.p.decide_proposals(&[p2.id], false).unwrap();
    let codings = w.p.list_codings(w.src).unwrap();
    assert_eq!(codings.len(), 1);
    assert_eq!(codings[0].origin, "ai_suggestion_accepted");
    assert_eq!(codings[0].ai_run_id, Some(packet.run_id));
    let run = w.p.ai_run(packet.run_id).unwrap().run;
    assert_eq!((run.pending, run.accepted, run.rejected), (0, 1, 1));
    // Each decision is one undo: the rejection first, then the acceptance, coding and all.
    w.p.undo().unwrap();
    assert_eq!(w.p.ai_run(packet.run_id).unwrap().run.pending, 1);
    assert_eq!(w.p.list_codings(w.src).unwrap().len(), 1);
    w.p.undo().unwrap();
    assert!(w.p.list_codings(w.src).unwrap().is_empty());
    assert_eq!(w.p.ai_run(packet.run_id).unwrap().run.pending, 2);
}

#[test]
fn an_answer_is_taken_once_and_needs_a_model_name_and_a_list() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let packet = w.p.build_packet(&spec(), w.ada).unwrap();
    let good = r#"[{"passage": "P1", "code": "Cost", "quote": "Fares", "reason": "fares"}]"#;

    assert!(w.p.answer_packet(packet.run_id, "   ", good).unwrap_err().to_string().contains("which model"));
    assert!(w.p.answer_packet(packet.run_id, "Bot", "").unwrap_err().to_string().contains("paste the answer"));
    assert!(w.p.answer_packet(packet.run_id, "Bot", "I could not do that.").unwrap_err().to_string().contains("no list"));
    assert!(w.p.answer_packet(packet.run_id, "Bot", "[{\"passage\": ").unwrap_err().to_string().contains("JSON"));
    assert!(w.p.answer_packet(9999, "Bot", good).is_err());
    assert!(!w.p.ai_run(packet.run_id).unwrap().run.answered, "a refused paste leaves the packet waiting for its answer");

    w.p.answer_packet(packet.run_id, "Bot", good).unwrap();
    assert!(w.p.answer_packet(packet.run_id, "Bot", good).unwrap_err().to_string().contains("already has an answer"));

    // An empty list is a valid answer: the model found nothing.
    let second = w.p.build_packet(&spec(), w.ada).unwrap();
    let none = w.p.answer_packet(second.run_id, "Bot", "[]").unwrap();
    assert_eq!((none.read, none.suggested, none.batch), (0, 0, None));
}

#[test]
fn suggestions_that_repeat_earlier_work_are_left_out_and_counted() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let s = start_of("too high");
    w.p.apply_coding(&NewCoding::new(w.src, s, s + 8, w.cost, w.ada)).unwrap();
    // Passages already coded are normally left out of the packet, so ask for all of them.
    let packet = w.p.build_packet(&PacketSpec { only_uncoded: false, ..spec() }, w.ada).unwrap();
    let answer = r#"[{"passage": "P1", "code": "Cost", "quote": "too high"}, {"passage": "P2", "code": "Access", "quote": "far from my house"}]"#;
    let outcome = w.p.answer_packet(packet.run_id, "Bot", answer).unwrap();
    assert_eq!((outcome.suggested, outcome.already_coded, outcome.already_waiting), (1, 1, 0));

    let again = w.p.build_packet(&PacketSpec { only_uncoded: false, ..spec() }, w.ada).unwrap();
    let outcome = w.p.answer_packet(again.run_id, "Bot", answer).unwrap();
    assert_eq!((outcome.suggested, outcome.already_coded, outcome.already_waiting), (0, 1, 1));
}

#[test]
fn a_code_retired_after_the_packet_was_built_is_refused_and_the_rest_still_goes_through() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    let packet = w.p.build_packet(&spec(), w.ada).unwrap();
    w.p.retire_code(w.access).unwrap();
    let answer = r#"[{"passage": "P1", "code": "Cost", "quote": "Fares"}, {"passage": "P2", "code": "Access", "quote": "The stop"}]"#;
    let outcome = w.p.answer_packet(packet.run_id, "Bot", answer).unwrap();
    assert_eq!((outcome.suggested, outcome.rejected.len()), (1, 1));
    assert!(outcome.rejected[0].why.contains("Access"));
}

#[test]
fn the_disclosure_paragraph_is_drawn_from_the_runs() {
    let dir = TempDir::new().unwrap();
    let mut w = world(&dir);
    assert_eq!(w.p.ai_disclosure().unwrap(), "No AI was used in this project.");

    let packet = w.p.build_packet(&spec(), w.ada).unwrap();
    let answer = r#"[{"passage": "P1", "code": "Cost", "quote": "Fares"}, {"passage": "P3", "code": "Cost", "quote": "nonsense"}]"#;
    w.p.answer_packet(packet.run_id, "Test Chatbot 1.0", answer).unwrap();
    let id = w.p.list_proposals(Some("pending"), None, 10).unwrap()[0].id;
    w.p.decide_proposals(&[id], true).unwrap();

    let text = w.p.ai_disclosure().unwrap();
    assert!(text.contains("only through prompt packets"), "{text}");
    assert!(text.contains("1 packet was built") && text.contains("1 was answered (by Test Chatbot 1.0)"), "{text}");
    assert!(text.contains("The answers held 2 suggested codings, of which 1 passed"), "{text}");
    assert!(text.contains("accepted 1 and rejected 0; 0 still wait"), "{text}");
    assert!(!text.contains('—'), "no em dashes");
}
