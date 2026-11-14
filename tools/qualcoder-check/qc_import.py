"""Imports a QDPX into a new QualCoder 4.0 project, headless, using QualCoder's own importer (refi.RefiImport), and dumps the project.

usage: python qc_import.py <file.qdpx> <new-project-folder.qda> <dump.json>
QualCoder's settings go to a throwaway home folder, never to the real one.
"""
import builtins
import datetime
import json
import os
import shutil
import sqlite3
import sys
import traceback

CHECK_DIR = os.environ.get("QUALCODER_CHECK_DIR") or os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "qualcoder-test"))
HOME = os.path.join(CHECK_DIR, "home")
os.makedirs(HOME, exist_ok=True)
os.environ["USERPROFILE"] = HOME
os.environ["HOME"] = HOME
os.environ["QT_QPA_PLATFORM"] = "offscreen"
sys.path.insert(0, os.path.join(CHECK_DIR, "QualCoder", "src"))
builtins._ = lambda s: s  # the translation function QualCoder installs

from PyQt6 import QtWidgets  # noqa: E402

qt_app = QtWidgets.QApplication([])

import qualcoder.refi as refi  # noqa: E402
from qualcoder.app import App  # noqa: E402

sys.excepthook = sys.__excepthook__  # QualCoder's own hook opens a window and waits; a headless run must just stop
qdpx, project_path, dump_path = sys.argv[1], sys.argv[2], sys.argv[3]

SCHEMA = [
    "CREATE TABLE project (databaseversion text, date text, memo text,about text, bookmarkfile integer, bookmarkpos integer, codername text, recently_used_codes text, avbookmarkfile integer, avbookmarkmsec integer, avbookmarktextpos integer)",
    "CREATE TABLE source (id integer primary key, name text, fulltext text, mediapath text, memo text, owner text, date text, av_text_id integer, risid integer, unique(name))",
    "CREATE TABLE code_image (imid integer primary key,id integer,x1 integer, y1 integer, width integer, height integer, cid integer, memo text, date text, owner text, important integer, pdf_page integer)",
    "CREATE TABLE code_av (avid integer primary key,id integer,pos0 integer, pos1 integer, cid integer, memo text, date text, owner text, important integer)",
    "CREATE TABLE annotation (anid integer primary key, fid integer,pos0 integer, pos1 integer, memo text, owner text, date text, unique(fid,pos0,pos1,owner))",
    "CREATE TABLE attribute_type (name text primary key, date text, owner text, memo text, caseOrFile text, valuetype text)",
    "CREATE TABLE attribute (attrid integer primary key, name text, attr_type text, value text, id integer, date text, owner text, unique(name,attr_type,id))",
    "CREATE TABLE case_text (id integer primary key, caseid integer, fid integer, pos0 integer, pos1 integer, owner text, date text, memo text)",
    "CREATE TABLE cases (caseid integer primary key, name text, memo text, owner text,date text, constraint ucm unique(name))",
    "CREATE TABLE code_cat (catid integer primary key, name text, owner text, date text, memo text, supercatid integer, unique(name))",
    "CREATE TABLE code_text (ctid integer primary key, cid integer, fid integer,seltext text, pos0 integer, pos1 integer, owner text, date text, memo text, avid integer, important integer, unique(cid,fid,pos0,pos1, owner))",
    "CREATE TABLE code_name (cid integer primary key, name text, memo text, catid integer, owner text,date text, color text, supercid integer, unique(name))",
    "CREATE TABLE journal (jid integer primary key, name text, jentry text, date text, owner text, unique(name))",
    "CREATE TABLE stored_sql (title text, description text, grouper text, ssql text, unique(title))",
    "CREATE TABLE graph (grid integer primary key, name text, description text, date text, scene_width integer, scene_height integer, unique(name));",
    "CREATE TABLE gr_cdct_text_item (gtextid integer primary key, grid integer, x integer, y integer, supercatid integer, catid integer, cid integer, font_size integer, bold integer, isvisible integer, displaytext text);",
    "CREATE TABLE gr_case_text_item (gcaseid integer primary key, grid integer, x integer, y integer, caseid integer, font_size integer, bold integer, color text, displaytext text);",
    "CREATE TABLE gr_file_text_item (gfileid integer primary key, grid integer, x integer, y integer, fid integer, font_size integer, bold integer, color text, displaytext text);",
    "CREATE TABLE gr_free_text_item (gfreeid integer primary key, grid integer, freetextid integer,x integer, y integer, free_text text, font_size integer, bold integer, color text,tooltip text, ctid integer,memo_ctid integer, memo_imid integer, memo_avid integer);",
    "CREATE TABLE gr_cdct_line_item (glineid integer primary key, grid integer, fromcatid integer, fromcid integer, tocatid integer, tocid integer, color text, linewidth real, linetype text, isvisible integer, label text, arrow_mode text);",
    "CREATE TABLE gr_free_line_item (gflineid integer primary key, grid integer, fromfreetextid integer, fromcatid integer, fromcid integer, fromcaseid integer,fromfileid integer, fromimid integer, fromavid integer, tofreetextid integer, tocatid integer, tocid integer, tocaseid integer, tofileid integer, toimid integer, toavid integer, color text,linewidth real, linetype text, label text, arrow_mode text);",
    "CREATE TABLE gr_memo_item (gmemoid integer primary key, grid integer, memo_source_type text, memo_source_id integer, x integer, y integer, color text, font_size integer);",
    "CREATE TABLE gr_pix_item (grpixid integer primary key, grid integer, imid integer,x integer, y integer, px integer, py integer, w integer, h integer, filepath text,tooltip text, pdf_page integer);",
    "CREATE TABLE gr_av_item (gr_avid integer primary key, grid integer, avid integer,x integer, y integer, pos0 integer, pos1 integer, filepath text, tooltip text, color text);",
    "CREATE TABLE ris (risid integer, tag text, longtag text, value text);",
    "CREATE TABLE manage_files_display (mfid integer primary key, name text, tblrows text, tblcolumns text, owner text);",
    "CREATE TABLE files_filter (filterid integer primary key, name text, filter text, owner text);",
]


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
shutil.rmtree(project_path, ignore_errors=True)
for sub in ("", "images", "audio", "video", "documents"):
    os.makedirs(os.path.join(project_path, sub), exist_ok=True)

app = App()
app.settings["codername"] = "Ada Lovelace"
app.create_connection(project_path)
cur = app.conn.cursor()
for sql in SCHEMA:
    cur.execute(sql)
app.update_coder_names()
cur.execute("INSERT INTO project VALUES(?,?,?,?,?,?,?,?,null,null,null)",
            ("v17", datetime.datetime.now().astimezone().strftime("%Y-%m-%d %H:%M:%S"), "", app.version, 0, 0, app.settings["codername"], ""))
app.conn.commit()

text = Text()
imp = refi.RefiImport.__new__(refi.RefiImport)
for k, v in dict(app=app, parent_textedit=text, import_type="qdpx", file_path=qdpx, folder_name="", sources_name="/Sources", file_vars=[], annotations=[], links=[],
                 tree=None, codes=[], users=[], cases=[], sources=[], source_name_prefix=0, variables=[], xml=None, base_path="", software_name="",
                 xmlns_version="1.0", pd=None, pd_value=0).items():
    setattr(imp, k, v)
error = None
try:
    imp.import_project()
except Exception:
    error = traceback.format_exc()

app.conn.commit()
out = {"error": error, "log": text.lines}
c = app.conn.cursor()
for table in ("project", "source", "code_cat", "code_name", "code_text", "code_image", "code_av", "annotation", "journal", "cases", "case_text",
              "attribute_type", "attribute", "graph", "gr_cdct_text_item", "gr_free_text_item", "gr_cdct_line_item", "gr_free_line_item", "coder_names"):
    try:
        c.execute(f"select * from {table}")
        cols = [d[0] for d in c.description]
        rows = [dict(zip(cols, r)) for r in c.fetchall()]
        if table == "source":
            for r in rows:
                r["fulltext_len"] = len(r["fulltext"] or "")
                r["fulltext_has_cr"] = "\r" in (r["fulltext"] or "")
                r["fulltext_head"] = (r["fulltext"] or "")[:60]
                del r["fulltext"]
        out[table] = rows
    except Exception as e:  # noqa
        out[table] = f"ERR {e}"
with open(dump_path, "w", encoding="utf-8") as f:
    json.dump(out, f, ensure_ascii=False, indent=1, default=str)
print("dump written", dump_path, "error:", bool(error))
if error:
    print(error)
app.conn.close()
