"""A second, independent reader for a QDPX file: pyrefiqda (MIT), whose models are generated from the standard's own schema.

    python tools/qdpx/second_reader.py <file.qdpx> [<folder to unzip into>]

It needs `pip install pyrefiqda` in a throwaway environment (nothing of PrismQDA's is imported). It prints what it found, so a file
PrismQDA wrote can be compared with what the project holds, and it exits with 1 if the reader cannot parse the file at all. It is a
second opinion on the shape of the file, in addition to `validate-qde.ps1` (the schema); neither says how another program would show it.
"""
import sys
import tempfile
from collections import Counter
from pathlib import Path

from pyrefiqda import RefiProject


def count(items):
    return len(items) if items is not None else 0


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    target = Path(sys.argv[1])
    folder = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(tempfile.mkdtemp(prefix="qdpx-second-reader-"))
    project = RefiProject.load(target, extract_dir=str(folder))
    print(f"read {target.name}: project '{project.name}', origin '{project.origin}'")
    print(f"  users: {count(project.users.user) if project.users else 0}")
    if project.users:
        for u in project.users.user:
            print(f"    - {u.name} ({u.guid})")

    def walk(codes):
        total = 0
        for c in codes or []:
            total += 1 + walk(getattr(c, "code", None))
        return total

    cb = project.code_book
    print(f"  codes (all levels): {walk(cb.codes.code) if cb and cb.codes else 0}")

    sources = project.sources
    kinds = Counter()
    selections = Counter()
    coders = Counter()
    codings = 0
    if sources:
        # The field names are the ones pyrefiqda made from the schema (the PDF one is `pdfsource`, with `pdfselection` under it).
        for kind in type(sources).model_fields:
            items = getattr(sources, kind, None) or []
            kinds[kind] = len(items)
            for s in items:
                for sel_attr in type(s).model_fields:
                    if not sel_attr.endswith("selection"):
                        continue
                    for sel in getattr(s, sel_attr, None) or []:
                        selections[sel_attr] += 1
                        coders[sel.creating_user] += 1
                        codings += len(getattr(sel, "coding", None) or [])
    print(f"  sources: {dict(kinds)}")
    print(f"  selections: {dict(selections)}  codings under them: {codings}")
    print(f"  selections by creatingUser guid: {dict(coders)}")
    for attr, label in (("cases", "cases"), ("variables", "variables"), ("notes", "notes"), ("links", "links"), ("sets", "sets"), ("graphs", "graphs")):
        v = getattr(project, attr, None)
        if v is None:
            print(f"  {label}: none")
        else:
            inner = [getattr(v, n) for n in type(v).model_fields if isinstance(getattr(v, n, None), list)]
            print(f"  {label}: {sum(len(i) for i in inner)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
