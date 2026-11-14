"""Exports a QualCoder 4.0 project as QDPX, headless, with QualCoder's own exporter (refi.RefiExport).

usage: python qc_export.py <project-folder.qda> <output-folder>
The two questions QualCoder asks in windows (line endings, where to save) are answered here: Windows line endings NOT added, and the folder given.
"""
import builtins
import os
import sys
import traceback

CHECK_DIR = os.environ.get("QUALCODER_CHECK_DIR") or os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "qualcoder-test"))
HOME = os.path.join(CHECK_DIR, "home")
os.environ["USERPROFILE"] = HOME
os.environ["HOME"] = HOME
os.environ["QT_QPA_PLATFORM"] = "offscreen"
sys.path.insert(0, os.path.join(CHECK_DIR, "QualCoder", "src"))
builtins._ = lambda s: s

from PyQt6 import QtWidgets  # noqa: E402

qt_app = QtWidgets.QApplication([])

import qualcoder.refi as refi  # noqa: E402
from qualcoder.app import App  # noqa: E402

sys.excepthook = sys.__excepthook__
project, outdir = sys.argv[1], sys.argv[2]
os.makedirs(outdir, exist_ok=True)
add_crlf = len(sys.argv) > 3 and sys.argv[3] == "crlf"


class Box:
    def __init__(self, *a, **k):
        pass

    def exec(self):
        return 0


class Radio:
    def __init__(self, on):
        self.on = on

    def isChecked(self):
        return self.on


class LineEndings:
    def __init__(self, app, parent=None):
        self.ui = type("U", (), {"radioButton_maxqda": Radio(add_crlf)})()

    def exec(self):
        return 0


class Text:
    def __init__(self):
        self.lines = []

    def append(self, s):
        self.lines.append(str(s))


refi.Message = Box
refi.RefiLineEndings = LineEndings
refi.QtWidgets.QFileDialog.getExistingDirectory = staticmethod(lambda *a, **k: outdir)

app = App()
app.settings["codername"] = "Ada Lovelace"
app.create_connection(project)
app.project_name = os.path.basename(project)
text = Text()
try:
    refi.RefiExport(app, text, "project")
    print("exported to", outdir)
except Exception:
    traceback.print_exc()
for l in text.lines:
    print(" ", l[:200])
