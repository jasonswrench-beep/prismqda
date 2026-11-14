# Tries the recording tools in the installed program's real window (WebView2 inside the Tauri shell): the waveform of a recording larger
# than 4 MiB, which the window reads through its own address under its content security policy, and a coded stretch saved as a WAV file.
#
#   powershell -ExecutionPolicy Bypass -File tools\installer-check\run-real-recording.ps1
#
# It plays nothing: the waveform and the clip are decoded, never played. Like the other checks here it stops (exit 3) unless the computer has
# been idle for ten minutes, because it opens PrismQDA's window, and stops (exit 4) if a PrismQDA is already running. It drives the window over
# WebView2's debugging port, with no mouse or keyboard of Windows, and the window's own data goes to a scratch folder.
param([string]$Scratch = (Join-Path $env:TEMP 'prismqda-recording-check'))
$ErrorActionPreference = 'Continue'
(Get-Process -Id $PID).ProcessorAffinity = [IntPtr]4294901760
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $env:PLAYWRIGHT_DIR) { $env:PLAYWRIGHT_DIR = Join-Path $env:USERPROFILE 'playwright-tools\node_modules\playwright' }   # where Playwright is installed (the browser check uses the same variable)
$passed = 0; $failed = 0
function Check($ok, $what) { if ($ok) { $script:passed++; "ok    $what" } else { $script:failed++; "FAIL  $what" } }

Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class IdleRecording { [StructLayout(LayoutKind.Sequential)] struct LASTINPUTINFO { public uint cbSize; public uint dwTime; }
 [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LASTINPUTINFO p);
 public static double Seconds() { var i = new LASTINPUTINFO(); i.cbSize = (uint)Marshal.SizeOf(i); GetLastInputInfo(ref i); return ((uint)Environment.TickCount - i.dwTime) / 1000.0; } }
'@
$idle = [IdleRecording]::Seconds()
"idle for $([int]$idle) seconds"
if ($idle -lt 600 -and $env:PRISMQDA_IDLE_OK -ne '1') { 'The computer is in use (idle under 10 minutes): NOT opening the window. (The person at the computer can say go ahead: set PRISMQDA_IDLE_OK=1 and keep hands off.)'; exit 3 }
if (Get-Process -Name 'prismqda-app' -ErrorAction SilentlyContinue) { 'A PrismQDA is already running: NOT starting another (it would be handed to that window).'; exit 4 }

$installedExe = Join-Path $env:LOCALAPPDATA 'PrismQDA\prismqda-app.exe'
Check (Test-Path $installedExe) "the program is installed at $installedExe"
Remove-Item $Scratch -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $Scratch 'project') | Out-Null
& node (Join-Path $here 'make_recording_project.mjs') (Join-Path $Scratch 'project')
$project = Join-Path $Scratch 'project\recording.pqda'
Check (Test-Path $project) 'the project with a recording larger than 4 MiB was made'

$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $Scratch 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9336 --remote-allow-origins=*'
$app = Start-Process -FilePath $installedExe -ArgumentList ('"' + $project + '"') -PassThru
$up = $false
for ($i = 0; $i -lt 80 -and -not $up; $i++) {
    try { $r = Invoke-WebRequest -UseBasicParsing -Uri 'http://127.0.0.1:9336/json/version' -TimeoutSec 2; $up = ($r.StatusCode -eq 200) } catch { Start-Sleep -Milliseconds 500 }
}
Check $up 'PrismQDA started on the project (its window answers on the debugging port)'
if ($up) {
    & node (Join-Path $here 'recording_probe.mjs') (Join-Path $Scratch 'clips')
    Check ($LASTEXITCODE -eq 0) 'the recording tools worked in the real window'
    [void]$app.CloseMainWindow()
    if (-not $app.WaitForExit(20000)) { "  had to stop pid $($app.Id) (it did not close)"; Stop-Process -Id $app.Id -Force }
} else {
    Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
}
# Windows may keep a short-lived helper process with the program's name for a moment after the window has closed, so wait for it.
for ($i = 0; $i -lt 30 -and (Get-Process -Name 'prismqda-app' -ErrorAction SilentlyContinue); $i++) { Start-Sleep -Milliseconds 500 }
Check (-not (Get-Process -Name 'prismqda-app' -ErrorAction SilentlyContinue)) 'PrismQDA closed'
$wal = Get-ChildItem (Join-Path $Scratch 'project') -Filter '*-wal' -ErrorAction SilentlyContinue | Where-Object { $_.Length -gt 0 }
Check (-not $wal) 'no project file was left with unsaved work in a -wal file'
"passed $passed, failed $failed"
exit ([int]($failed -gt 0))
