# Reading a QDPX file written by ELAN

ELAN (the Max Planck Institute's free annotation tool) writes a REFI-QDA project from *File > Export Multiple Files As > REFI Project File...*; it has no QDPX import.
These scripts make ELAN's **own writer** produce a file without opening its window, so that PrismQDA's reading of a real foreign file can be tried and kept as a test
(decision 179; what was found is in [docs/EXCHANGING-PROJECTS.md](../../docs/EXCHANGING-PROJECTS.md), the ELAN section).

## What has to be there (nothing is installed by these scripts)

- ELAN 7.1 for Windows, the **zip** from <https://archive.mpi.nl/tla/elan/download> (`ELAN_7-1_win.zip`; GPL-3), unpacked in its own folder: `<elan folder>\elan`.
  Its bundled Java (`ELAN_7.1\runtime\bin\java.exe`) is the one used to run the writer. The source (`ELAN_7-1_src.zip`) is only for reading how the writer is called.
- A JDK with `javac` (Temurin 25 was used), to compile the one small class here against ELAN's jars.
- The paths are at the top of each command below; change them if the folders are elsewhere.

## Run it

```powershell
# 1. invented material: an annotation file and a 20-second recording beside it
python tools\elan-check\make_eaf.py "<elan folder>\work"

# 2. compile the driver against ELAN's own jars (once)
$e = '<elan folder>\elan\ELAN_7-1_win\ELAN_7.1'
javac --release 25 -cp "$e\app\*" -d <elan folder>\classes tools\elan-check\ElanRefiExport.java

# 3. ELAN's writer: <in.eaf, or several separated by ;> <out.qdpx> <bundle|link> <transcript tiers> <coding tiers>
& "$e\runtime\bin\java.exe" -Djava.awt.headless=true -cp "$e\app\*;<elan folder>\classes" ElanRefiExport `
   "<elan folder>\work\bus-ride-talk.eaf" "<elan folder>\work\from-elan.qdpx" link "Interviewer,Participant" "Theme,Safety"
```

Then read the file in PrismQDA (`import.qdpx`, or `tools/qualcoder-check/pq_import.py <file.qdpx> <new project path without ending> <dump.json>` through the development bridge), and
check it with `tools/qdpx/validate-qde.ps1` (unzip the project file out first) and `tools/qdpx/second_reader.py`.

Run everything pinned to the efficiency cores of this computer (see `tools/qualcoder-check/pin.ps1`).

## What it is, and is not

It is ELAN's writer class called by a small program of ours, with the settings the export window's second step would set. It is **not** the window: the window's wizard
was not driven, so anything that only the window does is untried. The file the tests keep (`crates/prismqda-export/tests/fixtures/elan-7.1-bus-ride-talk.qdpx`) is what this
produced, with the folder of the recording changed to a neutral one.
