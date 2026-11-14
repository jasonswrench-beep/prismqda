# Opens PrismQDA's QDPX file in QualCoder 4.0 and QualCoder's file in PrismQDA, headless, and compares what comes through.
# Read README.md first: it says what has to be installed and which paths to change at the top of the scripts.
#
#   powershell -ExecutionPolicy Bypass -File tools\qualcoder-check\loop.ps1 <a-name-for-this-run>
#
$ErrorActionPreference = "Continue"
$here = $PSScriptRoot
$repo = (Resolve-Path "$here\..\..").Path
$d = if ($env:QUALCODER_CHECK_DIR) { $env:QUALCODER_CHECK_DIR } else { Join-Path (Split-Path -Parent $repo) 'qualcoder-test' }          # where QualCoder's source, its Python environment and the work folder are
$py = "$d\venv\Scripts\python.exe"
$pin = "$here\pin.ps1"                               # runs a program on the efficiency cores only (this computer's performance cores crash Python)
$tag = $args[0]
Set-Location $repo
& powershell -ExecutionPolicy Bypass -File $pin cargo build -p prismqda-cli --features dev-server 2>&1 | Select-Object -Last 1
Set-Location $d
Remove-Item "$d\work\from-qualcoder" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "$d\work\pq-from-qualcoder.pqda" -Force -ErrorAction SilentlyContinue
"== build the test project and export it from PrismQDA"
& powershell -ExecutionPolicy Bypass -File $pin $py "$here\build_project.py" 2>&1 | Select-String -Pattern '"export"|failed|Traceback'
"== QualCoder imports"
& powershell -ExecutionPolicy Bypass -File $pin $py "$here\qc_import.py" "$d\work\from-prismqda.qdpx" "$d\work\qc-from-prismqda.qda" "$d\work\qc-dump-$tag.json" 2>&1 | Select-String -Pattern 'dump written|Error|Traceback'
"== compare (what QualCoder holds)"
& $py -I "$here\compare.py" "$d\work\truth.json" "$d\work\qc-dump-$tag.json"
"== the PDF areas made the way QualCoder holds them (top-left)"
& $py -I "$here\qc_native_pdf.py" "$d\work\qc-from-prismqda.qda"
"== QualCoder exports"
& powershell -ExecutionPolicy Bypass -File $pin $py "$here\qc_export.py" "$d\work\qc-from-prismqda.qda" "$d\work\from-qualcoder" 2>&1 | Select-String -Pattern 'exported|Traceback|Error'
"== PrismQDA imports QualCoder's file"
& powershell -ExecutionPolicy Bypass -File $pin $py "$here\pq_import.py" "$d\work\from-qualcoder\qc-from-prismqda.qdpx" "$d\work\pq-from-qualcoder" "$d\work\pq-dump-$tag.json" 2>&1
"== compare (what PrismQDA holds after the trip)"
& $py -I "$here\compare2.py" "$d\work\truth.json" "$d\work\pq-dump-$tag.json"
