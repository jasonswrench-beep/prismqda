#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Checks that the files PrismQDA hands out carry a valid signature and a timestamp.

  python tools/check_signatures.py SETUP.exe                 the setup file and every program inside it
  python tools/check_signatures.py --zip PORTABLE.zip        every program in the portable zip
  python tools/check_signatures.py SETUP.exe --zip ZIP       both

A setup file made by Tauri is unpacked with 7-Zip into a temporary folder and each .exe, .dll and .pyd in it is read with Windows' own check
(the one File Properties > Digital Signatures shows), through tools/sign_windows.py. "Ours" means signed by OPENSTAGE_SIGN_PUBLISHER, or, when that
is not set, by whoever signed the setup file itself: ours must be valid AND timestamped (an Artifact Signing certificate lasts three days, so
a signature without a timestamp stops validating). A program someone else signed (Microsoft's, for example) must be valid and is left alone.
The uninstaller is written out only when the program is installed, so it is not among the unpacked files: check it after an install with
`python tools/check_signatures.py --installed` (it reads %LOCALAPPDATA%\\PrismQDA).

Exit code 0 when everything passes, 1 when something does not, 2 when it could not run.
"""

import os
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sign_windows  # noqa: E402

SEVEN_ZIP = Path(os.environ.get("OPENSTAGE_7ZIP") or r"C:\Program Files\7-Zip\7z.exe")


def program_files(folder):
    return sorted(p for p in Path(folder).rglob("*") if p.is_file() and p.suffix.lower() in sign_windows.PROGRAM_FILES)


def judge(found, base, publisher):
    """Sorts the signatures of the files under `base` into problems (in words) and a tally. `found` is sign_windows.signatures()."""
    problems, ours, makers = [], 0, {}
    for info in found:
        place = Path(info["path"]).relative_to(base).as_posix()
        if info["status"] == "Valid" and info["signer"] != publisher:
            makers[info["signer"]] = makers.get(info["signer"], 0) + 1
            continue
        why = sign_windows.problem(info, publisher)
        if why:
            problems.append(f"{place}: {why}")
        else:
            ours += 1
    return problems, ours, makers


def tally(label, ours, makers, publisher):
    others = "".join(f"; {n} by {name}" for name, n in sorted(makers.items()))
    print(f"  {label}: {ours} signed and timestamped by {publisher or 'nobody'}{others}", flush=True)


def check_setup(setup):
    setup = Path(setup).resolve()
    if not SEVEN_ZIP.is_file():
        return [f"7-Zip is not at {SEVEN_ZIP}, so the programs inside the setup cannot be checked (set OPENSTAGE_7ZIP)."]
    with tempfile.TemporaryDirectory(prefix="prismqda-signcheck-") as unpacked:
        unpacked = Path(unpacked).resolve()
        done = subprocess.run([str(SEVEN_ZIP), "x", "-y", f"-o{unpacked}", str(setup)], capture_output=True, text=True, encoding="utf-8", errors="replace")
        if done.returncode != 0:
            return ["7-Zip could not unpack the setup:\n" + done.stdout + done.stderr]
        inside = program_files(unpacked)
        found = sign_windows.signatures([setup, *inside])
        publisher = os.environ.get("OPENSTAGE_SIGN_PUBLISHER") or found[0]["signer"]
        problems = []
        why = sign_windows.problem(found[0], publisher)
        if why:
            problems.append(f"the setup file {setup.name}: {why}")
        if not inside:
            problems.append("no program files were found inside the setup (7-Zip unpacked nothing it could read)")
        inner, ours, makers = judge(found[1:], unpacked, publisher)
        problems += inner
        print(f"{setup.name}: signer of the setup {found[0]['signer'] or 'nobody'}; {len(inside)} program files inside", flush=True)
        tally("inside", ours, makers, publisher)
        if not any(p.name.lower().startswith("uninstall") for p in inside):
            print("  the uninstaller is written out when the program is installed: check it after an install (--installed).", flush=True)
        return problems


def check_zip(archive):
    archive = Path(archive).resolve()
    with tempfile.TemporaryDirectory(prefix="prismqda-signcheck-") as unpacked:
        unpacked = Path(unpacked).resolve()
        with zipfile.ZipFile(archive) as z:
            z.extractall(unpacked)
        inside = program_files(unpacked)
        found = sign_windows.signatures(inside)
        publisher = os.environ.get("OPENSTAGE_SIGN_PUBLISHER") or (found[0]["signer"] if found else None)
        problems = [] if inside else ["no program files were found in the zip"]
        inner, ours, makers = judge(found, unpacked, publisher)
        print(f"{archive.name}: {len(inside)} program files", flush=True)
        tally("in the zip", ours, makers, publisher)
        return problems + inner


def check_installed():
    folder = Path(os.environ.get("LOCALAPPDATA", "")) / "PrismQDA"
    if not folder.is_dir():
        return [f"PrismQDA is not installed at {folder}."]
    inside = program_files(folder)
    found = sign_windows.signatures(inside)
    publisher = os.environ.get("OPENSTAGE_SIGN_PUBLISHER") or (found[0]["signer"] if found else None)
    problems = [] if inside else ["no program files were found in the installed folder"]
    inner, ours, makers = judge(found, folder, publisher)
    print(f"installed in {folder}: {len(inside)} program files", flush=True)
    tally("installed", ours, makers, publisher)
    if not any(p.name.lower().startswith("uninstall") for p in inside):
        problems.append("the uninstaller is not in the installed folder")
    return problems + inner


def main(argv):
    args = list(argv)
    zip_path = None
    if "--zip" in args:
        at = args.index("--zip")
        if at + 1 >= len(args):
            print("--zip needs the zip file after it.")
            return 2
        zip_path = args[at + 1]
        del args[at:at + 2]
    installed = "--installed" in args
    if installed:
        args.remove("--installed")
    if not args and not zip_path and not installed:
        print(__doc__)
        return 2
    for p in [*args, *([zip_path] if zip_path else [])]:
        if not Path(p).is_file():
            print(f"There is no file at {p}.")
            return 2
    problems = []
    try:
        for setup in args:
            problems += check_setup(setup)
        if zip_path:
            problems += check_zip(zip_path)
        if installed:
            problems += check_installed()
    except (sign_windows.SignError, zipfile.BadZipFile) as e:
        print("Could not check: " + str(e))
        return 2
    for p in problems:
        print("  PROBLEM  " + p)
    print("Every program is signed and timestamped as it should be." if not problems else f"{len(problems)} problems.")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
