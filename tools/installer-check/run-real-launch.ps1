# Checks the installed PrismQDA the way a person double-clicking a project file would meet it, in its real window (WebView2 inside the Tauri shell).
#
#   powershell -ExecutionPolicy Bypass -File tools\installer-check\run-real-launch.ps1 [-Installer <the setup .exe to install first>]
#
# It stops (exit 3) unless the computer has been idle for ten minutes, because it opens PrismQDA's window; and it stops (exit 4) if a PrismQDA
# is already running, because a second start would be handed to that window (decision 178) instead of the one under test.
# With -Installer it first runs that installer silently over the installed copy (a per-user install under %LOCALAPPDATA%\PrismQDA).
# The window is driven only through its debugging port (launch_probe.mjs); no mouse or keyboard of the operating system is used,
# and the window's own data goes to a scratch folder, so the person's settings and recent list are not touched.
# What it checks: the file type is registered; a double click opens the project; a second start hands its file to the open window and ends;
# the project already open is not opened again; an encrypted project asks for its passphrase; a file that is not a project is said in words.
param(
    [string]$Installer = '',
    [string]$Scratch = (Join-Path $env:TEMP 'prismqda-launch-check')
)
$ErrorActionPreference = 'Continue'
(Get-Process -Id $PID).ProcessorAffinity = [IntPtr]4294901760   # the efficiency cores, as every check here
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $env:PLAYWRIGHT_DIR) { $env:PLAYWRIGHT_DIR = Join-Path $env:USERPROFILE 'playwright-tools\node_modules\playwright' }   # where Playwright is installed (the browser check uses the same variable)
$passed = 0; $failed = 0
function Check($ok, $what) { if ($ok) { $script:passed++; "ok    $what" } else { $script:failed++; "FAIL  $what" } }

Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class IdleLaunch { [StructLayout(LayoutKind.Sequential)] struct LASTINPUTINFO { public uint cbSize; public uint dwTime; }
 [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LASTINPUTINFO p);
 public static double Seconds() { var i = new LASTINPUTINFO(); i.cbSize = (uint)Marshal.SizeOf(i); GetLastInputInfo(ref i); return ((uint)Environment.TickCount - i.dwTime) / 1000.0; } }
'@
$idle = [IdleLaunch]::Seconds()
"idle seconds: $([Math]::Round($idle))"
if ($idle -lt 600 -and $env:PRISMQDA_IDLE_OK -ne '1') { 'The computer is in use (idle under 10 minutes): NOT opening the window. (The person at the computer can say go ahead: set PRISMQDA_IDLE_OK=1 and keep hands off.)'; exit 3 }
if (Get-Process -Name 'prismqda-app' -ErrorAction SilentlyContinue) { 'A PrismQDA is already running: NOT starting another (it would be handed to that window).'; exit 4 }

$installedExe = Join-Path $env:LOCALAPPDATA 'PrismQDA\prismqda-app.exe'
if ($Installer) {
    "== installing $Installer silently"
    $p = Start-Process -FilePath $Installer -ArgumentList '/S' -PassThru -Wait
    Check ($p.ExitCode -eq 0) "the installer ended with exit code $($p.ExitCode)"
}
Check (Test-Path $installedExe) "the program is installed at $installedExe"

"== the file type"
$ext = (& reg query 'HKCU\Software\Classes\.pqda' /ve 2>&1) -join ' '
$prog = if ($ext -match 'REG_SZ\s+(\S.*)$') { $Matches[1].Trim() } else { '' }
"  .pqda -> '$prog'"
Check ($prog -ne '') '.pqda is registered for the current user'
if ($prog) {
    $cmd = (& reg query "HKCU\Software\Classes\$prog\shell\open\command" /ve 2>&1) -join ' '
    "  open command: $cmd"
    Check ($cmd -match 'prismqda-app\.exe' -and $cmd -match '%1') 'its open command is the installed program with the file as the argument'
}

"== the projects to double click"
$projects = Join-Path $Scratch 'projects'
Remove-Item $Scratch -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $projects | Out-Null
& node (Join-Path $here 'make_projects.mjs') $projects
$alpha = Join-Path $projects 'alpha.pqda'; $beta = Join-Path $projects 'beta.pqda'
$secret = Join-Path $projects 'secret.pqda'; $broken = Join-Path $projects 'broken.pqda'

$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $Scratch 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9335 --remote-allow-origins=*'
function Probe($phase, $name) { & node (Join-Path $here 'launch_probe.mjs') $phase $name; Check ($LASTEXITCODE -eq 0) "window: $phase $name" }
function MainProcesses { @(Get-Process -Name 'prismqda-app' -ErrorAction SilentlyContinue) }

"== 1. double click a project with PrismQDA closed (through the file type, as Explorer does)"
Start-Process -FilePath $alpha
$up = $false
for ($i = 0; $i -lt 80 -and -not $up; $i++) {
    try { $r = Invoke-WebRequest -UseBasicParsing -Uri 'http://127.0.0.1:9335/json/version' -TimeoutSec 2; $up = ($r.StatusCode -eq 200) } catch { Start-Sleep -Milliseconds 500 }
}
Check $up 'PrismQDA started from the file (its window answers on the debugging port)'
$first = MainProcesses
Check ($first.Count -eq 1) "one PrismQDA process is running ($($first.Count))"
if ($up) {
    Probe 'open' 'Alpha One'

    "== 2. double click a second project while it is open: the same window shows it, no second program stays"
    $second = Start-Process -FilePath $installedExe -ArgumentList ('"' + $beta + '"') -PassThru
    $ended = $second.WaitForExit(20000)
    Check $ended 'the second start ended by itself'
    Check ((MainProcesses).Count -eq 1) "still one PrismQDA process ($((MainProcesses).Count))"
    Probe 'open' 'Beta Two'

    "== 3. double click the project that is open"
    $third = Start-Process -FilePath $installedExe -ArgumentList ('"' + $beta + '"') -PassThru
    [void]$third.WaitForExit(20000)
    Probe 'already' 'Beta Two'

    "== 4. double click an encrypted project (through the file type)"
    Start-Process -FilePath $secret
    Probe 'passphrase' ''

    "== 5. double click a file that is not a project (through the file type)"
    Start-Process -FilePath $broken
    Probe 'failed' 'Beta Two'

    "== 6. back to the first project"
    Start-Process -FilePath $alpha
    Probe 'open' 'Alpha One'

    "== closing the window the way the X button does"
    foreach ($m in $first) {
        [void]$m.CloseMainWindow()
        if (-not $m.WaitForExit(20000)) { "  had to stop pid $($m.Id) (it did not close)"; Stop-Process -Id $m.Id -Force }
    }
    Check ((MainProcesses).Count -eq 0) 'PrismQDA closed'
    $wal = Get-ChildItem $projects -Filter '*-wal' -ErrorAction SilentlyContinue | Where-Object { $_.Length -gt 0 }
    Check (-not $wal) 'no project file was left with unsaved work in a -wal file'
} else {
    foreach ($m in (MainProcesses)) { Stop-Process -Id $m.Id -Force }
}
"passed $passed, failed $failed"
exit ([int]($failed -gt 0))
