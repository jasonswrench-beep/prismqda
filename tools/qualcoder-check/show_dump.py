import sys; sys.stdout.reconfigure(encoding="utf-8")
import json
import sys

d = json.load(open(sys.argv[1], encoding="utf-8"))
print("ERROR:", d["error"])
print("--- log")
for l in d["log"]:
    print("  ", l[:200])
for t in ("source", "code_cat", "code_name", "cases", "case_text", "attribute_type", "journal", "graph", "gr_cdct_text_item", "gr_free_text_item", "gr_cdct_line_item", "gr_free_line_item", "coder_names"):
    rows = d[t]
    print(f"--- {t}: {len(rows) if isinstance(rows, list) else rows}")
    if isinstance(rows, list):
        for r in rows[:30]:
            r = {k: v for k, v in r.items() if k not in ("date",) and v not in (None, "")}
            print("  ", json.dumps(r, ensure_ascii=False)[:230])
for t in ("code_text", "code_image", "code_av"):
    rows = d[t]
    print(f"--- {t}: {len(rows)}")
    for r in rows:
        r = {k: v for k, v in r.items() if k not in ("date", "memo") or v}
        print("  ", json.dumps(r, ensure_ascii=False)[:260])
