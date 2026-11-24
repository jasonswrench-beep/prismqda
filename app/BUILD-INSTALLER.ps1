# Builds the interface, the desktop app, and the Windows installer, then writes the installer's SHA-256.
#
#   powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1          an unsigned installer
#   powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1 -Sign    signed and timestamped with Azure Artifact Signing
#   powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1 -Sign -Updates   also signed for the Check for updates button
#
# The release guard (tools\check-release-commit.ps1, CLAUDE.md rule 3) runs first and stops the build unless the tree is clean (no changed and no
# untracked file), the version is the same in tauri.conf.json, Cargo.toml and Cargo.lock, and HEAD is the commit that carries the annotated tag
# v<version>. So: commit everything, `git tag -a v<version> -m "PrismQDA <version>"`, then build. It runs again after the build, and the build
# stops if anything moved. The one way past it is -TestBuild (the owner's choice, 2026-11-17): a throwaway setup from any tree, unsigned
# (it refuses -Sign and -Updates), with no stamp, no SHA-256 file and no latest.json, left as
# app\src-tauri\target\release\bundle\test\PrismQDA_<version>_x64-TEST-<time>.exe. It is never a release: do not publish, upload or hand it out.
# It replaces the program in target\release, so build the release again before BUILD-PORTABLE.ps1 (which then refuses the test program).
#   powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1 -TestBuild
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
# app\src-tauri\src\updates.rs) is empty or is not the public half of that key: such a setup could never update anything (the check is
# tools\check-update-keys.ps1, which also warns when no backup key, BACKUP_PUBKEY, is built in, and refuses a normal release signed with the backup key).
#
# -Rotation -PreviousTag <tag> (with -Updates): the one release that changes the keys (docs\UPDATES.md, "Rotating the keys"). It is signed with the
# BACKUP key (TAURI_SIGNING_PRIVATE_KEY_PATH names the backup key's file), which must be exactly the BACKUP_PUBKEY of the release <tag> that people have
# installed, and the source must bring in a new PUBKEY and a new BACKUP_PUBKEY.

param([switch]$Sign, [switch]$Updates, [string]$NotesFile = '', [string]$DownloadBaseUrl = '', [switch]$TestBuild, [switch]$Rotation, [string]$PreviousTag = '')

$ErrorActionPreference = 'Stop'
[System.Diagnostics.Process]::GetCurrentProcess().ProcessorAffinity = [IntPtr]4294901760

$app = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Split-Path -Parent $app
$dryRun = ($env:OPENSTAGE_SIGN_DRY_RUN -eq '1')

# The release guard (CLAUDE.md, rule 3; tools\check-release-commit.ps1): nothing is built unless the tree is clean and HEAD is the commit
# that already carries this version's tag, so the tag is the exact commit of the build. It is checked again after the build.
# -TestBuild is the one way past it (below): a throwaway setup that is never signed, never stamped and never named like a release.
if ($TestBuild) {
    if ($Sign -or $Updates) { throw '-TestBuild cannot be combined with -Sign or -Updates: a test build is never signed and never offered to anyone. Commit, tag and build the release instead.' }
    # The build below replaces the program, so any stamp for the old program is void.
    Remove-Item (Join-Path $app 'src-tauri\target\release\prismqda-build-commit.txt') -Force -ErrorAction SilentlyContinue
    'TEST BUILD: the release guard is skipped, so this setup is not tied to any commit or tag. It will not be signed or stamped, and it is not for release.'
} else {
    $release = & (Join-Path $root 'tools\check-release-commit.ps1')
}

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
    # The key about to sign must be the one the program built from this source trusts (tools\check-update-keys.ps1; for a rotation, the backup key
    # of the release people have installed). It reads only the public half.
    $keyCheck = @{ KeyFile = $updateKey }
    if ($Rotation) { $keyCheck.Rotation = $true; $keyCheck.PreviousTag = $PreviousTag }
    & (Join-Path $root 'tools\check-update-keys.ps1') @keyCheck
} elseif ($Rotation) {
    throw '-Rotation is for a release signed for the Check for updates button: use it together with -Updates'
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

if ($TestBuild) {
    # Only a file that cannot be mistaken for a release is left: another folder, and TEST in the name instead of "-setup". The setup Tauri
    # wrote under the release name, and the hash and update signature of any earlier setup of that name, are removed.
    $built = Get-ChildItem 'target\release\bundle\nsis' -Filter '*-setup.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $testDir = 'target\release\bundle\test'
    New-Item -ItemType Directory -Force $testDir | Out-Null
    $testName = ($built.Name -replace '-setup\.exe$', ('-TEST-' + (Get-Date).ToString('yyyyMMdd-HHmm') + '.exe'))
    $testFile = Join-Path (Resolve-Path $testDir) $testName
    Move-Item $built.FullName $testFile -Force
    Remove-Item "$($built.FullName).sha256.txt", "$($built.FullName).sig" -Force -ErrorAction SilentlyContinue
    $headShort = try { (& git -C $root rev-parse --short HEAD) } catch { 'unknown' }
    $changed = try { @(& git -C $root status --porcelain --untracked-files=all).Count } catch { 'unknown' }
    "Built TEST setup $testFile"
    "From HEAD $headShort with $changed uncommitted path(s). Not signed, no update signature, no stamp, no SHA-256 file."
    'Never publish it, upload it or give it to anyone as a release. It replaced the program in target\release, so run the release build again before BUILD-PORTABLE.ps1 packs anything.'
    return
}

# The guard again: if the build changed a tracked file, left a new file, or HEAD moved, this build is not the tagged commit.
try {
    $null = & (Join-Path $root 'tools\check-release-commit.ps1') -ExpectCommit $release.Commit
} catch {
    throw "$($_.Exception.Message)`nThis build does not belong to $($release.Tag): do not publish it or use it. If the tag was never pushed: delete it (git tag -d $($release.Tag)), commit, tag again, and rebuild. If it was pushed: a new version and a new tag."
}
# The stamp BUILD-PORTABLE.ps1 reads, so a zip is only ever made from the program of this commit.
$exeHash = (Get-FileHash 'target\release\prismqda-app.exe' -Algorithm SHA256).Hash.ToLower()
Set-Content -Path 'target\release\prismqda-build-commit.txt' -Encoding ascii -Value @("commit=$($release.Commit)", "tag=$($release.Tag)", "program-sha256=$exeHash")

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
