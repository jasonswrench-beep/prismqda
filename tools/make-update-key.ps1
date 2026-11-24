# Makes a key that signs PrismQDA's updates. Run it once per key, yourself, in a terminal: it asks for a password (twice) that you choose.
#
#   powershell -ExecutionPolicy Bypass -File tools\make-update-key.ps1            the MAIN key: it signs every release
#   powershell -ExecutionPolicy Bypass -File tools\make-update-key.ps1 -Backup    the BACKUP key: kept offline, and used for one release only (a key rotation)
#
# What it makes: a private key file and its public half, in a folder outside the repository (by default PrismQDA-update-key, or
# PrismQDA-update-key-backup with -Backup, in your profile folder). The public half is put into app\src-tauri\src\updates.rs (PUBKEY for the main
# key, BACKUP_PUBKEY for the backup key), so the program built from then on trusts files signed with it. Check the change with `git diff`, and commit it.
#
# THE PRIVATE KEY AND ITS PASSWORD ARE THE ONLY WAY TO SIGN AN UPDATE. Keep the folder out of the repository, out of any sync folder that others
# can read, and back it up (a second copy on another drive or in a password manager). If the main key or its password is lost, the programs that
# are already installed can still be updated, once, by a release signed with the backup key (docs\UPDATES.md, "Rotating the keys"); with no backup
# key built in, they can never be updated again (a person would have to install a new setup file by hand once). If someone else gets a key and its
# password, they could sign an update that people's programs would accept: rotate at once. This script never prints a private key.
#
# The backup key is different in four ways, and the script enforces them: it never goes into PUBKEY; its folder must not be the main key's folder
# (a backup beside the main key protects against nothing); the main key must already be set; and it is never used to sign a normal release
# (BUILD-INSTALLER.ps1 refuses a normal build signed with it).
#
# Parameters, for testing only: -Folder (where the key goes), -NoSource (leave updates.rs alone), -SourcePath (another copy of updates.rs), -Password
# (skip the question; a password on a command line can be read by other programs, so do not use it for a real key).

param([string]$Folder = '', [switch]$Backup, [switch]$NoSource, [string]$Password = $null, [string]$SourcePath = '')

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$tauriCli = Join-Path $root 'app\ui\node_modules\.bin\tauri.cmd'
if (-not $SourcePath) { $SourcePath = Join-Path $root 'app\src-tauri\src\updates.rs' }
if (-not $Folder) { $Folder = Join-Path $env:USERPROFILE $(if ($Backup) { 'PrismQDA-update-key-backup' } else { 'PrismQDA-update-key' }) }

# Is the first path the same as, or inside, the second?
function Test-Inside([string]$Inner, [string]$Outer) {
    $a = [System.IO.Path]::GetFullPath($Inner).TrimEnd('\') + '\'
    $b = [System.IO.Path]::GetFullPath($Outer).TrimEnd('\') + '\'
    return $a.StartsWith($b, [System.StringComparison]::OrdinalIgnoreCase)
}

if ((Test-Inside $Folder $root) -or (Test-Inside $root $Folder)) { throw 'The key folder must not be inside the repository (or hold it)' }
$keyName = if ($Backup) { 'prismqda-update-backup.key' } else { 'prismqda-update.key' }
$key = Join-Path $Folder $keyName
if (Test-Path $key) { throw "There is already a key at $key. It is not replaced (replacing it would stop every installed program from updating). Move it away yourself if you mean to." }
if (Test-Path "$key.pub") { throw "There is already a public key at $key.pub. Move it away yourself if you mean to." }

$text = [System.IO.File]::ReadAllText($SourcePath)
if ($Backup) {
    if ((Test-Inside $Folder (Join-Path $env:USERPROFILE 'PrismQDA-update-key')) -or (Test-Path (Join-Path $Folder 'prismqda-update.key'))) {
        throw 'The backup key must not be kept in the main key''s folder: a backup beside the main key protects against nothing. Use another folder, and then take it off this computer.'
    }
    if ($text -notmatch 'pub const PUBKEY: &str = "([^"]*)";') { throw 'PUBKEY was not found in updates.rs' }
    if (-not $Matches[1].Trim()) { throw 'The main key is not set yet (PUBKEY in updates.rs is empty). Make the main key first.' }
    $mainPublic = $Matches[1].Trim()
}
if (-not (Test-Path $tauriCli)) { throw 'Tauri''s command line is not installed: run npm ci in app\ui first' }
New-Item -ItemType Directory -Force $Folder | Out-Null

if ($Password) {
    & $tauriCli signer generate --ci -w $key -p $Password
} else {
    & $tauriCli signer generate -w $key
}
if ($LASTEXITCODE -or -not (Test-Path "$key.pub")) { throw 'the key was not made' }

$public = (Get-Content "$key.pub" -Raw).Trim()
if ($Backup -and $public -eq $mainPublic) { throw 'The new key is the main key. It was not put anywhere.' }
if ($Backup) { "The BACKUP public key (safe to publish):" } else { "The public key (safe to publish):" }
$public
""
if ($NoSource) {
    "Not put into updates.rs (-NoSource)."
} else {
    $constant = if ($Backup) { 'BACKUP_PUBKEY' } else { 'PUBKEY' }
    $pattern = 'pub const ' + $constant + ': &str = "[^"]*";'
    if ($text -notmatch $pattern) { throw "$constant was not found in updates.rs" }
    $replaced = [regex]::Replace($text, $pattern, ('pub const ' + $constant + ': &str = "' + $public + '";'))
    [System.IO.File]::WriteAllText($SourcePath, $replaced, (New-Object System.Text.UTF8Encoding $false))
    "Put into $constant in $SourcePath. Review it with git diff, then commit it."
}
""
if ($Backup) {
    "Now: TAKE THIS KEY OFF THIS COMPUTER. Copy $Folder to offline storage (a USB stick kept in a drawer), check that the copy opens, put the"
    "password in your password manager, test the password once by signing a small file, and only then remove $Folder from this computer."
    "Never use this key for a normal release: it signs one release, the rotation (docs\UPDATES.md, 'Rotating the keys')."
} else {
    "Now: BACK UP $Folder (the private key and its password). Then build a release with"
    "  `$env:TAURI_SIGNING_PRIVATE_KEY_PATH = '$key'"
    "  `$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '<the password>'"
    "  powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1 -Sign -Updates"
}
