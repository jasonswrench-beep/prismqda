"""Builds a rich PrismQDA project (the made-up Urban Mobility Study plus extras) through the engine bridge and exports it as QDPX.

Everything here is invented data. The bridge is started on its own port, pinned to the efficiency cores, and stopped by its exact PID.
"""
import ctypes
import json
import os
import shutil
import socket
import subprocess
import sys
import time
import urllib.request
import wave

from PIL import Image, ImageDraw

REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
EXE = REPO + r"\target\debug\prismqda.exe"
WORK = os.path.join(os.environ.get("QUALCODER_CHECK_DIR") or os.path.join(os.path.dirname(REPO), "qualcoder-test"), "work")
PDF_FIXTURE = REPO + r"\crates\prismqda-import\tests\fixtures\pdf\three_pages.pdf"


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


class Bridge:
    def __init__(self):
        self.port = free_port()
        self.proc = subprocess.Popen([EXE, "--actor", "Ada Lovelace", "serve", "--port", str(self.port)], stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        # efficiency cores only (CPUs 16-31)
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
        body = json.dumps({"cmd": cmd, "args": args if args is not None else {}}).encode()
        req = urllib.request.Request(f"http://127.0.0.1:{self.port}/rpc", data=body, headers={"Content-Type": "application/json"})
        out = json.loads(urllib.request.urlopen(req, timeout=120).read())
        if not out["ok"]:
            raise RuntimeError(f"{cmd}: {out['error']}")
        return out["result"]

    def stop(self):
        self.proc.kill()


def main():
    shutil.rmtree(WORK, ignore_errors=True)
    os.makedirs(WORK)
    b = Bridge()
    log = []
    try:
        rpc = b.rpc
        proj = os.path.join(WORK, "Urban Mobility Study.pqda")
        rpc("sample.create", {"id": "urban-mobility", "path": proj})
        rpc("project.open", {"path": proj, "backup": False})
        me = next(c for c in rpc("coder.list") if c["role"] == "human")
        sam = rpc("coder.add", {"name": "Sam Okafor", "role": "human"})
        code = {c["name"]: c["id"] for c in rpc("code.list")}
        log.append(("codes in the sample", sorted(code)))

        # ---- a document with an emoji, accents, CJK and right-to-left text, and Windows line endings
        interview = (
            "Interview with a made-up participant\r\n\r\n"
            "I felt welcome from the first day. The staff greeted me by name. \U0001F600 Cost was the hard part for me.\r\n\r\n"
            "Caf\u00e9 owner said: Na\u00efve plans never survive. \u6211\u611f\u5230\u5f88\u53d7\u6b22\u8fce\u3002 \u05e9\u05dc\u05d5\u05dd \u05e2\u05d5\u05dc\u05dd is a greeting. We kept q\u0301 apart.\r\n\r\n"
            "This is the last paragraph. Everything here is invented text for testing the screens.\r\n"
        )
        p = os.path.join(WORK, "Unicode interview.txt")
        with open(p, "w", encoding="utf-8", newline="") as f:
            f.write(interview)
        done = rpc("import.run", {"path": p})
        uid = done["outcome"]["source_ids"][0]
        src = rpc("source.get", {"id": uid})
        stored = src.get("text") or src.get("full_text") or ""
        log.append(("unicode doc keys", sorted(src.keys())))
        text = rpc("text.read", {"source_id": uid}) if False else None
        # offsets are counted in characters of the stored text; find them there
        def span(s, sub):
            i = s.index(sub)
            return i, i + len(sub)
        base = stored if stored else interview
        for sub, cname in [("Cost was the hard part for me.", "Cost"), ("Naïve plans never survive.", "Reliability"), ("שלום עולם is a greeting.", "Accessibility"), ("Everything here is invented", "Safety")]:
            try:
                a, z = span(base, sub)
                rpc("coding.apply", {"source_id": uid, "start": a, "end": z, "code_id": code[cname], "coder_id": me["id"]})
                rpc("coding.apply", {"source_id": uid, "start": a, "end": z, "code_id": code[cname], "coder_id": sam["id"]})
            except Exception as e:  # noqa
                log.append(("coding failed", sub, str(e)))
        log.append(("unicode text stored has CR?", "\r" in base, len(base), len(interview)))

        # ---- a picture with a rectangle, an ellipse and an outline, by two coders
        img = Image.new("RGB", (800, 500), (200, 225, 250))
        d = ImageDraw.Draw(img)
        d.rectangle([60, 238, 260, 350], fill=(120, 130, 140))
        d.ellipse([470, 270, 720, 370], fill=(240, 180, 50))
        d.polygon([(300, 210), (340, 210), (350, 260), (290, 260)], fill=(30, 90, 170))
        png = os.path.join(WORK, "Bus stop.png")
        img.save(png)
        pid = rpc("import.run", {"path": png})["outcome"]["source_ids"][0]
        rpc("region.add", {"source_id": pid, "shape": "rectangle", "x": 0.075, "y": 0.476, "w": 0.25, "h": 0.224, "code_id": code["Safety"], "coder_id": me["id"], "rationale": "A shelter."})
        rpc("region.add", {"source_id": pid, "shape": "ellipse", "x": 0.5875, "y": 0.54, "w": 0.3125, "h": 0.2, "code_id": code["Reliability"], "coder_id": me["id"]})
        rpc("region.add", {"source_id": pid, "shape": "polygon", "points": [[0.375, 0.42], [0.425, 0.42], [0.4375, 0.52], [0.3625, 0.52]], "code_id": code["Accessibility"], "coder_id": me["id"]})
        rpc("region.add", {"source_id": pid, "shape": "rectangle", "x": 0.08, "y": 0.48, "w": 0.24, "h": 0.22, "code_id": code["Safety"], "coder_id": sam["id"]})

        # ---- a PDF with a rectangle
        pdf = os.path.join(WORK, "Garden notes.pdf")
        shutil.copy(PDF_FIXTURE, pdf)
        fid = rpc("import.run", {"path": pdf})["outcome"]["source_ids"][0]
        rpc("region.add", {"source_id": fid, "page": 1, "shape": "rectangle", "x": 0.1, "y": 0.12, "w": 0.8, "h": 0.1, "code_id": code["Cost"], "coder_id": me["id"], "rationale": "The title."})
        rpc("region.add", {"source_id": fid, "page": 2, "shape": "rectangle", "x": 0.2, "y": 0.3, "w": 0.5, "h": 0.2, "code_id": code["Reliability"], "coder_id": me["id"]})

        # ---- a recording with stretches
        wav = os.path.join(WORK, "Walking interview.wav")
        with wave.open(wav, "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(1)
            w.setframerate(8000)
            w.writeframes(bytes([128]) * 8000 * 20)
        rid = rpc("import.run", {"path": wav})["outcome"]["source_ids"][0]
        rpc("region.add", {"source_id": rid, "shape": "span", "start_ms": 2000, "end_ms": 6500, "code_id": code["Reliability"], "coder_id": me["id"], "rationale": "Missed a connection."})
        rpc("region.add", {"source_id": rid, "shape": "span", "start_ms": 9000, "end_ms": 14500, "code_id": code["Safety"], "coder_id": me["id"]})

        # ---- a timed transcript (caption file) linked to a second recording
        vtt = os.path.join(WORK, "Bus ride talk.vtt")
        with open(vtt, "w", encoding="utf-8", newline="") as f:
            f.write(
                "WEBVTT\n\n"
                "00:00:01.000 --> 00:00:05.000\nInterviewer: How do you get to work?\n\n"
                "00:00:05.500 --> 00:00:12.000\nParticipant: I take the bus, but it is often late and the fare keeps going up.\n\n"
                "00:00:12.500 --> 00:00:16.000\nInterviewer: What would help?\n\n"
                "00:00:16.500 --> 00:00:19.000\nParticipant: A shelter and a real timetable.\n"
            )
        tid = rpc("import.run", {"path": vtt})["outcome"]["source_ids"][0]
        wav2 = os.path.join(WORK, "Bus ride talk.wav")
        shutil.copy(wav, wav2)
        try:
            rpc("media.link", {"source_id": tid, "path": wav2})
        except Exception as e:  # noqa
            log.append(("media.link failed", str(e)))
        units = rpc("unit.list", {"source_id": tid})
        log.append(("timed units", [(u["start"], u["end"], u.get("time_start_ms"), u.get("time_end_ms")) for u in units[:4]]))
        for u in units:
            if u.get("speaker") == "Participant" or (u["end"] - u["start"] > 30):
                try:
                    rpc("coding.apply", {"source_id": tid, "start": u["start"], "end": u["end"], "code_id": code["Reliability"], "coder_id": me["id"], "unit_id": u["id"]})
                except Exception as e:  # noqa
                    log.append(("coding timed unit failed", str(e)))
                break

        # ---- a link between two passages, a concept map, a case with values
        sample_codings = []
        for s in rpc("source.list"):
            if s.get("codings"):
                sample_codings = rpc("coding.list", {"source_id": s["id"]})
                if len(sample_codings) >= 2:
                    break
        if len(sample_codings) >= 2:
            try:
                rpc("link.add", {"from_coding": sample_codings[0]["id"], "to_coding": sample_codings[1]["id"], "relation": "contradicts"})
            except Exception as e:  # noqa
                log.append(("link failed", str(e)))
        try:
            m = rpc("map.create", {"name": "How cost and reliability fit"})
            n1 = rpc("map.node_add", {"map_id": m["id"], "kind": "code", "ref_id": code["Cost"], "x": 100, "y": 100})
            n2 = rpc("map.node_add", {"map_id": m["id"], "kind": "code", "ref_id": code["Reliability"], "x": 350, "y": 100})
            rpc("map.link_add", {"map_id": m["id"], "from": n1["id"], "to": n2["id"], "label": "leads to"})
        except Exception as e:  # noqa
            log.append(("map failed", str(e)))

        # ---- variables of cases, one of each kind the file can hold (text, whole number, decimal, date)
        try:
            for name, level in (("Role", "nominal"), ("Age", "ratio"), ("Score", "ratio"), ("Joined", "nominal")):
                rpc("variable.define", {"name": name, "level": level})
            data = rpc("data.get")
            example = {"Role": ["driver", "rider"], "Age": ["34", "52"], "Score": ["3.5", "4.25"], "Joined": ["2021-03-04", "2019-11-30"]}
            for name, values in example.items():
                changes = [{"case_id": c["id"], "value": values[i % len(values)]} for i, c in enumerate(data["cases"])]
                rpc("variable.set_values", {"variable": name, "changes": changes})
        except Exception as e:  # noqa
            log.append(("variables failed", str(e)))

        # ---- the truth, for comparing with what another program makes of the file
        truth = {"coders": rpc("coder.list"), "codes": rpc("code.list"), "data": rpc("data.get"), "sources": []}
        for s in rpc("source.list"):
            det = rpc("source.get", {"id": s["id"]})
            entry = {"id": s["id"], "name": s["name"], "text": det.get("text", ""), "codings": rpc("coding.list", {"source_id": s["id"]}), "regions": []}
            try:
                entry["regions"] = rpc("region.list", {"source_id": s["id"]})
            except Exception:  # noqa
                pass
            truth["sources"].append(entry)
        with open(os.path.join(WORK, "truth.json"), "w", encoding="utf-8") as f:
            json.dump(truth, f, ensure_ascii=False, indent=1, default=str)

        # ---- export
        out = os.path.join(WORK, "from-prismqda")
        res = rpc("export.qdpx", {"path": out, "include_recordings": True})
        log.append(("export", res["path"], res["bytes"]))
        log.append(("carried", res["report"]["carried"]))
        log.append(("left out", res["report"]["left_out"]))
        info = rpc("source.list")
        log.append(("sources", [(s["id"], s["name"]) for s in info]))
        rpc("project.close", {})
    finally:
        b.stop()
    for entry in log:
        print(json.dumps(entry, ensure_ascii=False, default=str))


main()
