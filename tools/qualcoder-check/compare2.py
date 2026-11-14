"""Compares what PrismQDA first held (truth.json) with what it holds after a trip through another program (a dump from pq_import.py)."""
import json
import sys

sys.stdout.reconfigure(encoding="utf-8")
truth = json.load(open(sys.argv[1], encoding="utf-8"))
back = json.load(open(sys.argv[2], encoding="utf-8"))

tcode = {c["id"]: c["name"] for c in truth["codes"]}
tcoder = {c["id"]: c["name"] for c in truth["coders"]}
bcode = {c["id"]: c["name"] for c in back["codes"]}
bcoder = {c["id"]: c["name"] for c in back["coders"]}


def norm(n):
    return n[:-4] if n.lower().endswith(".pdf") else n


bsrc = {norm(s["name"]): s for s in back["sources"]}
print("coders now:", [c["name"] for c in back["coders"]])
print("sources now:", [s["name"] for s in back["sources"]])
print("codes: sent", len(truth["codes"]), "back", len(back["codes"]))
missing_text = 0
for s in truth["sources"]:
    b = bsrc.get(norm(s["name"]))
    if not b:
        print(f"SOURCE MISSING BACK: {s['name']}")
        continue
    same_text = b["text"] == s["text"]
    exp = {(c["start"], c["end"], tcode[c["code_id"]], tcoder[c["coder_id"]]) for c in s["codings"]}
    got = {(c["start"], c["end"], bcode[c["code_id"]], bcoder[c["coder_id"]]) for c in b["codings"]}
    flag = "" if (exp == got and same_text) else "   <<<"
    print(f"{s['name']}: text same={same_text} codings sent {len(exp)} back {len(got)}{flag}")
    for e in sorted(exp - got):
        print("     lost or changed:", e, repr(s["text"][e[0]:e[1]][:40]))
        missing_text += 1
    for g in sorted(got - exp):
        print("     new:", g)
    # regions
    rexp = sorted((r["shape"], r.get("page"), tcode[r["code_id"]], tcoder[r["coder_id"]], r.get("start_ms")) for r in s["regions"])
    rgot = sorted((r["shape"], r.get("page"), bcode[r["code_id"]], bcoder[r["coder_id"]], r.get("start_ms")) for r in b["regions"])
    if rexp or rgot:
        print(f"   regions sent {len(rexp)} back {len(rgot)}")
        for r in s["regions"]:
            print("      sent", r["shape"], r.get("page"), {k: r[k] for k in r if k in ("x", "y", "w", "h", "points", "start_ms", "end_ms")}, tcoder[r["coder_id"]], "|", r["rationale"])
        for r in b["regions"]:
            print("      back", r["shape"], r.get("page"), {k: r[k] for k in r if k in ("x", "y", "w", "h", "points", "start_ms", "end_ms")}, bcoder[r["coder_id"]], "|", r["rationale"])
print("memos back:", [(m.get("title"), m.get("scope")) for m in back["memos"]] if isinstance(back["memos"], list) else back["memos"])
print("cases back:", [(c.get("name"), c.get("source_count") or c.get("sources")) for c in back["cases"]] if isinstance(back["cases"], list) else back["cases"])
print("maps back:", back["maps"] if not isinstance(back["maps"], list) else len(back["maps"]))
print("links back:", back["links"] if not isinstance(back["links"], list) else len(back["links"]))
print("media back:", back["media"] if not isinstance(back["media"], list) else len(back["media"]))

# ---- the variables of cases after the round trip
sent = truth.get("data")
if sent and back.get("data"):
    after = {c["name"]: dict(zip([v["name"] for v in back["data"]["variables"]], row)) for c, row in zip(back["data"]["cases"], back["data"]["values"])}
    wrong = []
    for case, row in zip(sent["cases"], sent["values"]):
        for var, value in zip(sent["variables"], row):
            if value is not None and after.get(case["name"], {}).get(var["name"]) != value:
                wrong.append((case["name"], var["name"], value, after.get(case["name"], {}).get(var["name"])))
    print("variables of cases back:", [(v["name"], v["level"]) for v in back["data"]["variables"]], "| values changed on the way:", wrong if wrong else 0)
