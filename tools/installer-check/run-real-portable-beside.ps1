# Checks the portable copy next to the installed program, in their real windows (WebView2 inside the Tauri shell).
#
#   powershell -ExecutionPolicy Bypass -File tools\installer-check\run-real-portable-beside.ps1 [-Zip <the portable zip>]
#
# An installed PrismQDA is one program however many project files are opened (decision 178); the portable copy is not, so it can run beside it.
# This starts the installed program on one project, then the unzipped portable copy on another, and looks at both windows: two programs
# are running, each shows its own project, and the portable copy has written nothing to the Windows profile's file type list.
# It stops (exit 3) unless the computer has been idle for ten minutes, and stops (exit 4) if a PrismQDA is already running.
param(
    [string]$Zip = (Get-ChildItem (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'app\src-tauri\target\release\bundle') -Filter 'PrismQDA_*_portable.zip' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName),
    [string]$Scratch = (Join-Path $env:TEMP 'prismqda-portable-beside')
)
$ErrorActionPreference = 'Continue'
(Get-Process -Id $PID).ProcessorAffinity = [IntPtr]4294901760
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $env:PLAYWRIGHT_DIR) { $env:PLAYWRIGHT_DIR = Join-Path $env:USERPROFILE 'playwright-tools\node_modules\playwright' }   # where Playwright is installed (the browser check uses the same variable)
$passed = 0; $failed = 0
function Check($ok, $what) { if ($ok) { $script:passed++; "ok    $what" } else { $script:failed++; "FAIL  $what" } }

Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class IdleBeside { [StructLayout(LayoutKind.Sequential)] struct LASTINPUTINFO { public uint cbSize; public uint dwTime; }
 [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LASTINPUTINFO p);
 public static double Seconds() { var i = new LASTINPUTINFO(); i.cbSize = (uint)Marshal.SizeOf(i); GetLastInputInfo(ref i); return ((uint)Environment.TickCount - i.dwTime) / 1000.0; } }
'@
$idle = [IdleBeside]::Seconds()
"idle seconds: $([Math]::Round($idle))"
if ($idle -lt 600 -and $env:PRISMQDA_IDLE_OK -ne '1') { 'The computer is in use (idle under 10 minutes): NOT opening the windows. (The person at the computer can say go ahead: set PRISMQDA_IDLE_OK=1 and keep hands off.)'; exit 3 }
if (Get-Process -Name 'prismqda-app', 'PrismQDA' -ErrorAction SilentlyContinue) { 'A PrismQDA is already running: NOT starting more.'; exit 4 }

$installedExe = Join-Path $env:LOCALAPPDATA 'PrismQDA\prismqda-app.exe'
Remove-Item $Scratch -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $Scratch | Out-Null
Expand-Archive -Path $Zip -DestinationPath (Join-Path $Scratch 'unzipped')
$portableExe = Join-Path $Scratch 'unzipped\PrismQDA\PrismQDA.exe'
Check (Test-Path (Join-Path $Scratch 'unzipped\PrismQDA\portable.txt')) 'the portable copy has its marker beside the program'
$projects = Join-Path $Scratch 'projects'
New-Item -ItemType Directory -Force $projects | Out-Null
& node (Join-Path $here 'make_projects.mjs') $projects
$alpha = Join-Path $projects 'alpha.pqda'; $beta = Join-Path $projects 'beta.pqda'

function WaitPort($port) {
    for ($i = 0; $i -lt 80; $i++) {
        try { $r = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:$port/json/version" -TimeoutSec 2; if ($r.StatusCode -eq 200) { return $true } } catch { Start-Sleep -Milliseconds 500 }
    }
    return $false
}

"== the installed program on one project"
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $Scratch 'webview-installed'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9335 --remote-allow-origins=*'
$installed = Start-Process -FilePath $installedExe -ArgumentList ('"' + $alpha + '"') -PassThru
Check (WaitPort 9335) 'the installed program answers on its debugging port'
$env:PROBE_PORT = '9335'
& node (Join-Path $here 'launch_probe.mjs') open 'Alpha One'
Check ($LASTEXITCODE -eq 0) 'the installed program shows Alpha One'

"== the portable copy on another project, while the installed one is open"
Remove-Item Env:\WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue   # the portable copy keeps its own window data beside itself
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9336 --remote-allow-origins=*'
$portable = Start-Process -FilePath $portableExe -ArgumentList ('"' + $beta + '"') -PassThru
# Every start of the installed program during the next seconds (a start that hands its file over and ends is short-lived).
$seen = @{}
for ($i = 0; $i -lt 100; $i++) {
    foreach ($c in @(Get-CimInstance Win32_Process -Filter "Name='prismqda-app.exe'")) { if (-not $seen.ContainsKey($c.ProcessId)) { $seen[$c.ProcessId] = "parent $($c.ParentProcessId), command $($c.CommandLine)" } }
    Start-Sleep -Milliseconds 80
}
foreach ($k in $seen.Keys) { "    seen prismqda-app pid ${k}: $($seen[$k])" }
Check (WaitPort 9336) 'the portable copy answers on its own debugging port (it did not hand its file to the installed program)'
$env:PROBE_PORT = '9336'
& node (Join-Path $here 'launch_probe.mjs') open 'Beta Two'
Check ($LASTEXITCODE -eq 0) 'the portable copy shows Beta Two, started with a project file'
$env:PROBE_PORT = '9335'
& node (Join-Path $here 'launch_probe.mjs') open 'Alpha One'
Check ($LASTEXITCODE -eq 0) 'and the installed program still shows Alpha One'
$running = @(Get-Process -Name 'prismqda-app', 'PrismQDA' -ErrorAction SilentlyContinue)
"  running: " + (($running | ForEach-Object { "$($_.ProcessName) pid $($_.Id)" }) -join '; ')
foreach ($r in $running) { $c = Get-CimInstance Win32_Process -Filter "ProcessId=$($r.Id)"; "    pid $($r.Id): parent $($c.ParentProcessId), started $($r.StartTime.ToString('HH:mm:ss')), command $($c.CommandLine)" }
Check ($running.Count -eq 2) 'two programs are running'
Check (Test-Path (Join-Path $Scratch 'unzipped\PrismQDA\PrismQDAData')) 'the portable copy kept its data in PrismQDAData beside itself'

"== closing both windows the way the X button does"
foreach ($p in @($installed, $portable)) {
    [void]$p.CloseMainWindow()
    if (-not $p.WaitForExit(20000)) { "  had to stop pid $($p.Id)"; Stop-Process -Id $p.Id -Force }
}
Start-Sleep -Seconds 2
$left = @(Get-Process -Name 'prismqda-app', 'PrismQDA' -ErrorAction SilentlyContinue)
"  still running: " + (($left | ForEach-Object { "$($_.ProcessName) pid $($_.Id)" }) -join '; ')
Check ($left.Count -eq 0) 'both closed'
"passed $passed, failed $failed"
exit ([int]($failed -gt 0))
