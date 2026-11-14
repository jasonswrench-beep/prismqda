# Builds a portable copy of PrismQDA: a zip that holds the program and a marker file, and nothing to install.
#
#   powershell -ExecutionPolicy Bypass -File app\BUILD-PORTABLE.ps1
#
# It packs the program that BUILD-INSTALLER.ps1 built (app\src-tauri\target\release\prismqda-app.exe), so build the installer first
# when the program has changed; this script builds it for you only when there is no program yet. Everything is pinned to CPUs 16 to 31,
# the efficiency cores, as the installer script does.
#
# -Sign: the program in the zip is signed and timestamped with Azure Artifact Signing, through tools\sign_windows.py (a program the
# installer build already signed is left as it is), and tools\check_signatures.py checks the finished zip. It needs what
# BUILD-INSTALLER.ps1 -Sign needs (see the top of that file), and builds the program with -Sign if there is none yet. Without -Sign the
# zip is not signed.
#
# The result is app\src-tauri\target\release\bundle\PrismQDA_<version>_portable.zip plus a .sha256.txt beside it.
# Beside the program in the zip, portable.txt is the marker: a program with that file beside it keeps its settings, its list of recent
# projects, and the translations a person added in a folder called PrismQDAData beside it, not in the Windows profile.

param([switch]$Sign)

$ErrorActionPreference = 'Stop'
[System.Diagnostics.Process]::GetCurrentProcess().ProcessorAffinity = [IntPtr]4294901760

$app = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Split-Path -Parent $app
$dryRun = ($env:OPENSTAGE_SIGN_DRY_RUN -eq '1')
if ($Sign) {
    $python = (Get-Command python -ErrorAction SilentlyContinue).Source
    if (-not $python) { throw 'python was not found, and signing needs it (tools\sign_windows.py)' }
    if (-not $env:OPENSTAGE_SIGN_PUBLISHER) { throw 'OPENSTAGE_SIGN_PUBLISHER is not set: it names the signer the build must see on every file' }
    if (-not $dryRun) {
        & $python (Join-Path $root 'tools\sign_windows.py') --ready
        if ($LASTEXITCODE) { throw 'signing cannot start: see what is missing above' }
    }
}
$exe = Join-Path $app 'src-tauri\target\release\prismqda-app.exe'
if (-not (Test-Path $exe)) {
    & (Join-Path $app 'BUILD-INSTALLER.ps1') -Sign:$Sign
    if ($LASTEXITCODE) { throw 'the program did not build' }
}

$config = Get-Content (Join-Path $app 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$version = $config.version
$stage = Join-Path ([System.IO.Path]::GetTempPath()) ("prismqda-portable-" + [guid]::NewGuid().ToString('N'))
$folder = Join-Path $stage 'PrismQDA'
New-Item -ItemType Directory -Force $folder | Out-Null

Copy-Item $exe (Join-Path $folder 'PrismQDA.exe')
if ($Sign) {
    & $python (Join-Path $root 'tools\sign_windows.py') --description 'PrismQDA' (Join-Path $folder 'PrismQDA.exe')
    if ($LASTEXITCODE) { throw 'the program in the zip was not signed (see above)' }
}
foreach ($f in @('LICENSE', 'NOTICE', 'THIRD-PARTY.md')) { Copy-Item (Join-Path $root $f) (Join-Path $folder $f) }
Copy-Item (Join-Path $app 'ui\src\assets\fonts\Inter-OFL.txt') (Join-Path $folder 'Inter-OFL.txt')

# The marker. Its words say what it does, so a person who finds it knows.
Set-Content -Path (Join-Path $folder 'portable.txt') -Encoding ascii -Value @(
    'This file makes this copy of PrismQDA portable.',
    'While it is beside PrismQDA.exe, the program keeps its settings, the list of recent projects, and any translations you added',
    'in the folder PrismQDAData beside it, instead of in your Windows profile. Delete this file to use your Windows profile again.',
    'Your projects are the .pqda files you save, wherever you save them.'
)

$signedLine = if ($Sign) {
    '- The program is signed (right-click PrismQDA.exe, Properties, Digital Signatures). Windows SmartScreen may still say "Windows protected your PC" for a program it has not seen often; choose "More info", then "Run anyway".'
} else {
    '- The program is not signed, so Windows SmartScreen says "Windows protected your PC". Choose "More info", then "Run anyway".'
}
Set-Content -Path (Join-Path $folder 'README-PORTABLE.txt') -Encoding ascii -Value @(
    'PrismQDA, portable copy',
    '',
    'Unzip this folder anywhere, a folder on your computer or a USB stick, and run PrismQDA.exe. Nothing is installed and no',
    'administrator rights are needed.',
    '',
    '- Your settings, your list of recent projects, and any translations you added are kept in the folder PrismQDAData beside the',
    '  program (it is made the first time you run it). Copy the whole folder to take them with you.',
    '- Your projects are the .pqda files you save. Save them where you like. A project is one file; copy it to move it.',
    '- PrismQDA needs Microsoft WebView2, which Windows 11 includes. If it is missing, Windows will say so when you start the program.',
    $signedLine,
    '- The program connects to nothing on its own. The one exception is Check for updates in Settings, which you press; a portable copy is not updated in place, so it tells you to download the new zip.',
    '',
    'LICENSE, NOTICE, THIRD-PARTY.md, and Inter-OFL.txt are the licenses of PrismQDA and of what is in it.'
)

$bundle = Join-Path $app 'src-tauri\target\release\bundle'
New-Item -ItemType Directory -Force $bundle | Out-Null
$zip = Join-Path $bundle ("PrismQDA_" + $version + "_portable.zip")
if (Test-Path $zip) { Remove-Item $zip -Force }
# A program just signed may be held for a moment by the virus scanner, which makes Compress-Archive fail with "used by another process";
# try again a few times before giving up.
for ($try = 1; $try -le 5; $try++) {
    try {
        Compress-Archive -Path $folder -DestinationPath $zip -ErrorAction Stop
        break
    } catch {
        if ($try -eq 5) { throw }
        if (Test-Path $zip) { Remove-Item $zip -Force }
        Start-Sleep -Seconds 4
    }
}
Remove-Item $stage -Recurse -Force

if ($Sign -and -not $dryRun) {
    & $python (Join-Path $root 'tools\check_signatures.py') --zip $zip
    if ($LASTEXITCODE) { throw 'the zip was built, but its program is not signed as it should be (see above)' }
}
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
"$hash  $(Split-Path -Leaf $zip)" | Out-File -Encoding ascii "$zip.sha256.txt"
"Built $zip"
"SHA-256 $hash"
