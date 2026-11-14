# Builds the interface, the desktop app, and the Windows installer, then writes the installer's SHA-256.
#
#   powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1          an unsigned installer
#   powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1 -Sign    signed and timestamped with Azure Artifact Signing
#   powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1 -Sign -Updates   also signed for the Check for updates button
#
# Needs Rust, Node.js, and (on the first run) a network connection: npm packages, Rust crates, and Tauri's
# installer tools (NSIS) are downloaded once. Everything is pinned to CPUs 16 to 31, the efficiency cores,
# because the performance cores of the author's computer are unstable (see TEST PRISMQDA.bat).
# The result is app\src-tauri\target\release\bundle\nsis\PrismQDA_<version>_x64-setup.exe plus a .sha256.txt beside it.
#
# -Sign: Tauri is given tools\sign_windows.py as its signCommand (a temporary configuration beside the build, so the signing
# settings never go into tauri.conf.json or the repository). Tauri then signs the program, the installer's plugins, the
# uninstaller and the setup file, and tools\check_signatures.py checks the setup file and every program inside it before the
# SHA-256 is written, so the hash is the hash of the signed file. The build stops if anything is unsigned or untimestamped.
# What it needs (run `python tools\sign_windows.py --ready` to see what is missing):
#   OPENSTAGE_SIGN_ENDPOINT, OPENSTAGE_SIGN_ACCOUNT, OPENSTAGE_SIGN_PROFILE   the Artifact Signing account (not secrets; keep them
#                                                                              out of the repository)
#   OPENSTAGE_SIGN_PUBLISHER   the name the signature must show; a file signed as anyone else fails the check
#   a sign-in: `az login`, once (nothing secret is stored by these scripts)
#   OPENSTAGE_SIGN_DRY_RUN=1   prints the signing commands and signs nothing (the build then skips the check)
#
# -Updates: makes the finished setup file something the program's Check for updates button will accept (docs\UPDATES.md). After the setup has
# been built and checked, it is signed with the update key (the key tools\make-update-key.ps1 made; point TAURI_SIGNING_PRIVATE_KEY_PATH at its
# file, and TAURI_SIGNING_PRIVATE_KEY_PASSWORD at its password if it has one), and `latest.json` is written beside it: the version, the notes
# (-NotesFile, a text file), the day, the signature, and where the setup file will be (-DownloadBaseUrl, by default the release `v<version>` of the
# project's repository). The build stops if the key is not there, or if the public key built into the program (PUBKEY in
# app\src-tauri\src\updates.rs) is empty or is not the public half of that key: such a setup could never update anything.

param([switch]$Sign, [switch]$Updates, [string]$NotesFile = '', [string]$DownloadBaseUrl = '')

$ErrorActionPreference = 'Stop'
[System.Diagnostics.Process]::GetCurrentProcess().ProcessorAffinity = [IntPtr]4294901760

$app = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Split-Path -Parent $app
$dryRun = ($env:OPENSTAGE_SIGN_DRY_RUN -eq '1')

# Leaves the builder's folders out of the program. Rust writes the place of every error message into the program, as a full path, so a
# build would carry the Windows account's folder name (C:\Users\<name>\.cargo\...) and this project's folder. The two prefixes are
# replaced by short stand-ins. (Cargo's own `trim-paths` is not stable yet. The flags go in CARGO_ENCODED_RUSTFLAGS, separated by the
# character 31, because the project's folder has spaces in its name.)
$remap = @("--remap-path-prefix=$env:USERPROFILE=/user", "--remap-path-prefix=$root=/prismqda")
$env:CARGO_ENCODED_RUSTFLAGS = (@($env:CARGO_ENCODED_RUSTFLAGS) + $remap | Where-Object { $_ }) -join [string][char]31

$updateKey = $null
if ($Updates) {
    $updateKey = $env:TAURI_SIGNING_PRIVATE_KEY_PATH
    if (-not $updateKey -or -not (Test-Path $updateKey)) { throw 'TAURI_SIGNING_PRIVATE_KEY_PATH must name the update key file (tools\make-update-key.ps1 makes it)' }
    if (-not (Test-Path "$updateKey.pub")) { throw "the public half of the update key is not beside it ($updateKey.pub)" }
    $source = Get-Content (Join-Path $app 'src-tauri\src\updates.rs') -Raw
    if ($source -notmatch 'pub const PUBKEY: &str = "([^"]*)";') { throw 'PUBKEY was not found in updates.rs' }
    $built = $Matches[1].Trim()
    if (-not $built) { throw 'PUBKEY in app\src-tauri\src\updates.rs is empty, so the program would not check for updates at all (tools\make-update-key.ps1 puts it there)' }
    if ($built -ne (Get-Content "$updateKey.pub" -Raw).Trim()) { throw 'PUBKEY in updates.rs is not the public half of the update key; a program built like that would refuse every update' }
}

$signConfig = $null
if ($Sign) {
    $python = (Get-Command python -ErrorAction SilentlyContinue).Source
    if (-not $python) { throw 'python was not found, and signing needs it (tools\sign_windows.py)' }
    if (-not $env:OPENSTAGE_SIGN_PUBLISHER) { throw 'OPENSTAGE_SIGN_PUBLISHER is not set: it names the signer the build must see on every file' }
    if (-not $dryRun) {
        & $python (Join-Path $root 'tools\sign_windows.py') --ready
        if ($LASTEXITCODE) { throw 'signing cannot start: see what is missing above' }
    }
    $signDir = Join-Path $app 'src-tauri\target\signing'
    New-Item -ItemType Directory -Force $signDir | Out-Null
    $signConfig = Join-Path $signDir 'tauri.sign.conf.json'
    @{ bundle = @{ windows = @{ signCommand = @{ cmd = $python; args = @((Join-Path $root 'tools\sign_windows.py'), '--description', 'PrismQDA', '%1') } } } } |
        ConvertTo-Json -Depth 8 | Out-File -Encoding ascii $signConfig
}

Set-Location (Join-Path $app 'ui')
if (-not (Test-Path 'node_modules')) { npm ci --no-audit --no-fund; if ($LASTEXITCODE) { throw 'npm ci failed' } }
npm run build
if ($LASTEXITCODE) { throw 'the interface did not build' }

Set-Location (Join-Path $app 'src-tauri')
if ($Sign) {
    & '..\ui\node_modules\.bin\tauri.cmd' build --ci --config $signConfig
} else {
    & '..\ui\node_modules\.bin\tauri.cmd' build --ci
}
if ($LASTEXITCODE) { throw 'the app did not build' }

$installer = Get-ChildItem 'target\release\bundle\nsis' -Filter '*-setup.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($Sign -and -not $dryRun) {
    & $python (Join-Path $root 'tools\check_signatures.py') $installer.FullName
    if ($LASTEXITCODE) { throw 'the setup was built, but it is not signed as it should be (see above)' }
}
if ($Updates) {
    # After the code signature, because the update signature covers the exact bytes of the file people download.
    $tauriCli = Join-Path $app 'ui\node_modules\.bin\tauri.cmd'
    & $tauriCli signer sign -f $updateKey $installer.FullName
    if ($LASTEXITCODE) { throw 'the update signature could not be made' }
    $sigFile = "$($installer.FullName).sig"
    if (-not (Test-Path $sigFile)) { throw 'the update signature file was not written' }
    $config = Get-Content (Join-Path $app 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
    $version = $config.version
    $base = if ($DownloadBaseUrl) { $DownloadBaseUrl.TrimEnd('/') } else { "https://github.com/jasonswrench-beep/prismqda/releases/download/v$version" }
    $notes = if ($NotesFile) { (Get-Content $NotesFile -Raw).Trim() } else { "See the release notes for version $version." }
    $manifest = [ordered]@{
        version   = $version
        notes     = $notes
        pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
        platforms = [ordered]@{ 'windows-x86_64' = [ordered]@{ signature = (Get-Content $sigFile -Raw).Trim(); url = "$base/$($installer.Name)" } }
    }
    [System.IO.File]::WriteAllText((Join-Path $installer.DirectoryName 'latest.json'), ($manifest | ConvertTo-Json -Depth 6), (New-Object System.Text.UTF8Encoding $false))
    "Wrote $(Join-Path $installer.DirectoryName 'latest.json') (version $version, the setup at $base/$($installer.Name))"
}
$hash = (Get-FileHash $installer.FullName -Algorithm SHA256).Hash.ToLower()
"$hash  $($installer.Name)" | Out-File -Encoding ascii "$($installer.FullName).sha256.txt"
"Built $($installer.FullName)"
"SHA-256 $hash"
