"""Imports a QDPX into a new PrismQDA project (through the engine bridge) and dumps what arrived.

usage: python pq_import.py <file.qdpx> <new-project-path-without-ending> <dump.json>
"""
import ctypes
import json
import os
import socket
import subprocess
import sys
import time
import urllib.request

EXE = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "target", "debug", "prismqda.exe"))
sys.stdout.reconfigure(encoding="utf-8")


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
        body = json.dumps({"cmd": cmd, "args": args if args is not None else {}}).encode()
        req = urllib.request.Request(f"http://127.0.0.1:{self.port}/rpc", data=body, headers={"Content-Type": "application/json"})
        out = json.loads(urllib.request.urlopen(req, timeout=120).read())
        if not out["ok"]:
            raise RuntimeError(f"{cmd}: {out['error']}")
        return out["result"]

    def stop(self):
        self.proc.kill()


qdpx, target, dump = sys.argv[1], sys.argv[2], sys.argv[3]
b = Bridge()
try:
    rpc = b.rpc
    res = rpc("import.qdpx", {"path": qdpx, "project_path": target})
    print("REPORT carried:", json.dumps(res["report"].get("carried"), ensure_ascii=False))
    print("REPORT left out:", json.dumps(res["report"].get("left_out"), ensure_ascii=False))
    for k in res["report"]:
        if k not in ("carried", "left_out"):
            print("REPORT", k, json.dumps(res["report"][k], ensure_ascii=False)[:600])
    rpc("project.open", {"path": res["project_path"], "backup": False})
    out = {"project": rpc("project.info"), "coders": rpc("coder.list"), "codes": rpc("code.list"), "sources": []}
    for s in rpc("source.list"):
        det = rpc("source.get", {"id": s["id"]})
        e = {"id": s["id"], "name": s["name"], "summary": s, "text": det.get("text", ""), "codings": rpc("coding.list", {"source_id": s["id"]})}
        try:
            e["regions"] = rpc("region.list", {"source_id": s["id"]})
        except Exception as ex:  # noqa
            e["regions"] = []
        out["sources"].append(e)
    out["memos"] = rpc("memo.list", {})
    out["cases"] = rpc("case.list", {})
    out["data"] = rpc("data.get", {})
    try:
        out["maps"] = rpc("map.list", {})
    except Exception as ex:  # noqa
        out["maps"] = str(ex)
    try:
        out["links"] = rpc("link.list", {})
    except Exception as ex:  # noqa
        out["links"] = str(ex)
    try:
        out["media"] = rpc("media.list", {})
    except Exception as ex:  # noqa
        out["media"] = str(ex)
    rpc("project.close", {})
    with open(dump, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=1, default=str)
    print("dump written", dump)
finally:
    b.stop()
