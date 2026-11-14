"""Codebook exchange (QDC) both ways between PrismQDA and QualCoder 4.0."""
import builtins
import ctypes
import json
import os
import shutil
import socket
import subprocess
import sys
import time
import urllib.request

CHECK_DIR = os.environ.get("QUALCODER_CHECK_DIR") or os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "qualcoder-test"))
HOME = os.path.join(CHECK_DIR, "home")
os.environ["USERPROFILE"] = HOME
os.environ["HOME"] = HOME
os.environ["QT_QPA_PLATFORM"] = "offscreen"
sys.path.insert(0, os.path.join(CHECK_DIR, "QualCoder", "src"))
sys.stdout.reconfigure(encoding="utf-8")
builtins._ = lambda s: s

WORK = os.path.join(CHECK_DIR, "work", "qdc")
shutil.rmtree(WORK, ignore_errors=True)
os.makedirs(WORK)
EXE = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "target", "debug", "prismqda.exe"))


class Bridge:
    def __init__(self):
        s = socket.socket()
        s.bind(("127.0.0.1", 0))
        self.port = s.getsockname()[1]
        s.close()
        self.proc = subprocess.Popen([EXE, "--actor", "Ada Lovelace", "serve", "--port", str(self.port)], stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        ctypes.windll.kernel32.SetProcessAffinityMask(int(self.proc._handle), 0xFFFF0000)
        t0 = time.time()
        while time.time() - t0 < 20:
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{self.port}/health", timeout=1).read()
                return
            except Exception:
                time.sleep(0.1)
        raise RuntimeError("bridge did not start")

    def rpc(self, cmd, args=None):
        body = json.dumps({"cmd": cmd, "args": args or {}}).encode()
        req = urllib.request.Request(f"http://127.0.0.1:{self.port}/rpc", data=body, headers={"Content-Type": "application/json"})
        out = json.loads(urllib.request.urlopen(req, timeout=120).read())
        if not out["ok"]:
            raise RuntimeError(f"{cmd}: {out['error']}")
        return out["result"]

    def stop(self):
        self.proc.kill()


# ---- 1. PrismQDA writes a codebook, with a theme and a code with all of its text fields
b = Bridge()
try:
    r = b.rpc
    r("project.create", {"path": WORK + r"\a.pqda", "name": "Codebook out"})
    theme = r("code.add", {"name": "Getting around", "theme": True, "definition": "What people give up to travel."})
    r("code.add", {"name": "Cost", "parent_id": theme["id"], "color": "#f5a524", "definition": "Money spent.", "inclusion_criteria": "Says a price or a fare.", "exclusion_criteria": "Talks only about time."})
    r("code.add", {"name": "Reliability \u00e9", "parent_id": theme["id"], "definition": "Late or cancelled."})
    r("code.add", {"name": "Safety & trust", "definition": "Feeling safe."})
    out1 = r("export.codebook", {"format": "qdc", "path": WORK + r"\from-prismqda"})
    print("PrismQDA wrote", out1["path"])
finally:
    b.stop()

# ---- 2. QualCoder reads it
from PyQt6 import QtWidgets  # noqa: E402

qt_app = QtWidgets.QApplication([])
import qualcoder.refi as refi  # noqa: E402
from qualcoder.app import App  # noqa: E402

sys.excepthook = sys.__excepthook__
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


class Box:
    def __init__(self, *a, **k):
        pass

    def exec(self):
        return 0


class Text:
    def __init__(self):
        self.lines = []

    def append(self, s):
        self.lines.append(str(s))


refi.Message = Box
# the tables, copied from qc_import.py by reading its list
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "qc_import.py"), encoding="utf-8").read()
start = src.index("SCHEMA = [")
end = src.index("\n]\n", start) + 3
ns = {}
exec(src[start:end], ns)
SCHEMA = ns["SCHEMA"]

import datetime  # noqa: E402

proj = WORK + r"\qc.qda"
for sub in ("", "images", "audio", "video", "documents"):
    os.makedirs(os.path.join(proj, sub), exist_ok=True)
app = App()
app.settings["codername"] = "Ada Lovelace"
app.create_connection(proj)
cur = app.conn.cursor()
for sql in SCHEMA:
    cur.execute(sql)
app.update_coder_names()
cur.execute("INSERT INTO project VALUES(?,?,?,?,?,?,?,?,null,null,null)", ("v17", datetime.datetime.now().astimezone().strftime("%Y-%m-%d %H:%M:%S"), "", app.version, 0, 0, "Ada Lovelace", ""))
app.conn.commit()

text = Text()
imp = refi.RefiImport.__new__(refi.RefiImport)
for k, v in dict(app=app, parent_textedit=text, import_type="qdc", file_path=out1["path"], folder_name="", sources_name="/Sources", file_vars=[], annotations=[], links=[],
                 tree=None, codes=[], users=[], cases=[], sources=[], source_name_prefix=0, variables=[], xml=None, base_path="", software_name="",
                 xmlns_version="1.0", pd=None, pd_value=0).items():
    setattr(imp, k, v)
imp.import_codebook()
c = app.conn.cursor()
c.execute("select cid, name, memo, color, supercid, catid from code_name order by cid")
print("QUALCODER HOLDS (cid, name, memo, color, supercid, catid):")
for row in c.fetchall():
    print("   ", row)
c.execute("select catid, name, memo, supercatid from code_cat")
print("QUALCODER CATEGORIES:", c.fetchall())
print("log:", text.lines[-3:])

# ---- 3. QualCoder writes its codebook
outdir = WORK + r"\from-qualcoder"
os.makedirs(outdir, exist_ok=True)
refi.QtWidgets.QFileDialog.getExistingDirectory = staticmethod(lambda *a, **k: outdir)
app.project_name = "qc.qda"
try:
    refi.RefiExport(app, Text(), "codebook")
except Exception:
    import traceback

    traceback.print_exc()
print("QualCoder wrote:", os.listdir(outdir))
app.conn.close()

# ---- 4. PrismQDA reads that
qdc = os.path.join(outdir, os.listdir(outdir)[0])
b = Bridge()
try:
    r = b.rpc
    r("project.create", {"path": WORK + r"\b.pqda", "name": "Codebook in"})
    res = r("import.qdc", {"path": qdc})
    print("PrismQDA import report:", json.dumps(res, ensure_ascii=False)[:900])
    for c_ in r("code.list"):
        print("   ", c_["id"], c_["name"], "| parent", c_["parent_id"], "| kind", c_["kind"], "|", c_["definition"], "|", c_["inclusion_criteria"], "|", c_["exclusion_criteria"], "|", c_["color"])
finally:
    b.stop()
