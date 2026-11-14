"""Compares what PrismQDA sent (truth.json) with what QualCoder made of it (a dump from qc_import.py). Text, picture, PDF and recording codings."""
import json
import sys

sys.stdout.reconfigure(encoding="utf-8")
truth = json.load(open(sys.argv[1], encoding="utf-8"))
qc = json.load(open(sys.argv[2], encoding="utf-8"))

code_name = {c["id"]: c["name"] for c in truth["codes"]}
coder_name = {c["id"]: c["name"] for c in truth["coders"]}
qc_code = {c["cid"]: c["name"] for c in qc["code_name"]}
qc_source = {s["id"]: s for s in qc["source"]}
by_name = {}
for s in qc["source"]:
    by_name[s["name"]] = s

problems = 0

# ---- text
expected = set()
for s in truth["sources"]:
    for c in s["codings"]:
        expected.add((s["name"], c["start"], c["end"], code_name[c["code_id"]], coder_name[c["coder_id"]], s["text"][c["start"]:c["end"]]))
got = set()
for r in qc["code_text"]:
    src = qc_source[r["fid"]]
    got.add((src["name"], r["pos0"], r["pos1"], qc_code[r["cid"]], r["owner"], r["seltext"]))
print(f"TEXT: sent {len(expected)} codings; QualCoder holds {len(got)} text codings (it also adds transcript copies)")
for e in sorted(expected - got):
    problems += 1
    print("  NOT FOUND AS SENT:", e[:5], repr(e[5][:50]))
extra = sorted(got - expected)
for g in extra:
    print("  EXTRA/DIFFERENT IN QUALCODER:", g[:5], repr(g[5][:50]))

# ---- pictures and PDFs (rectangles)
W, H = 800, 500
for s in truth["sources"]:
    for r in s["regions"]:
        if r["shape"] == "span":
            continue
        tgt = by_name.get(s["name"]) or by_name.get(s["name"] + ".pdf")
        rows = [x for x in qc["code_image"] if tgt and x["id"] == tgt["id"]]
        print(f"AREA on {s['name']} ({r['shape']}, page {r.get('page')}, {coder_name[r['coder_id']]}, {code_name[r['code_id']]}):", "sent as", {k: r[k] for k in r if k in ('x', 'y', 'w', 'h', 'points')})
print("QualCoder code_image rows:")
for x in qc["code_image"]:
    print("   ", qc_source[x["id"]]["name"], qc_code[x["cid"]], x["owner"], (x["x1"], x["y1"], x["width"], x["height"]), "page", x["pdf_page"], "memo", x["memo"])

# ---- recordings
print("QualCoder code_av rows:")
for x in qc["code_av"]:
    print("   ", qc_source[x["id"]]["name"], qc_code[x["cid"]], x["owner"], (x["pos0"], x["pos1"]), "memo", x["memo"])
for s in truth["sources"]:
    for r in s["regions"]:
        if r["shape"] == "span":
            print("  sent span", s["name"], code_name[r["code_id"]], coder_name[r["coder_id"]], r.get("start_ms"), r.get("end_ms"))

print("\nSOURCES in QualCoder:", [(s["id"], s["name"], s["owner"], s["mediapath"]) for s in qc["source"]])
print("CASES:", [(c["name"], c["owner"]) for c in qc["cases"]], "case_text rows:", len(qc["case_text"]))
print("JOURNAL:", [(j["name"], j["owner"]) for j in qc["journal"]])
print("GRAPH tables:", {k: len(qc[k]) for k in ("graph", "gr_cdct_text_item", "gr_free_text_item", "gr_cdct_line_item", "gr_free_line_item")})
print("ANNOTATIONS:", len(qc["annotation"]))
print("PROBLEMS (text codings missing as sent):", problems)

# ---- the variables of cases: every value PrismQDA sent should be an attribute of the case in QualCoder
sent = truth.get("data")
if sent:
    case_id = {c["name"]: c["caseid"] for c in qc["cases"]}
    held = {(a["name"], a["id"]): a["value"] for a in qc["attribute"] if a["attr_type"] == "case"}
    kinds = {t["name"]: t["valuetype"] for t in qc["attribute_type"] if t["caseOrFile"] == "case"}
    missing = []
    for case, row in zip(sent["cases"], sent["values"]):
        for var, value in zip(sent["variables"], row):
            if value is None:
                continue
            got = held.get((var["name"], case_id.get(case["name"])))
            if got != value:
                missing.append((case["name"], var["name"], value, got))
    print("VARIABLES of cases in QualCoder:", {k: v for k, v in kinds.items()}, "| values sent but not held as sent:", missing if missing else 0)
