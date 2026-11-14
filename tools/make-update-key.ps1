# Makes the key that signs PrismQDA's updates. Run it once, yourself, in a terminal: it asks for a password (twice) that you choose.
#
#   powershell -ExecutionPolicy Bypass -File tools\make-update-key.ps1
#
# What it makes: a private key file and its public half, in a folder outside the repository (by default PrismQDA-update-key in your
# profile folder). The public half is put into app\src-tauri\src\updates.rs (PUBKEY), so the program built from then on trusts only
# files signed with this key. Check the change with `git diff`, and commit it.
#
# THE PRIVATE KEY AND ITS PASSWORD ARE THE ONLY WAY TO SIGN AN UPDATE. Keep the folder out of the repository, out of any sync folder that others
# can read, and back it up (a second copy on another drive or in a password manager). If the key or its password is lost, the programs that
# are already installed can never be updated again (a person would have to install a new setup file by hand once, and the program
# built after that would carry a new key). If someone else gets both, they could sign an update that people's programs would accept:
# then make a new key at once and publish a setup built with it. This script never prints the private key.
#
# Parameters, for testing only: -Folder (where the key goes), -NoSource (leave updates.rs alone), -Password (skip the question; a password on a
# command line can be read by other programs, so do not use it for the real key).

param([string]$Folder = (Join-Path $env:USERPROFILE 'PrismQDA-update-key'), [switch]$NoSource, [string]$Password = $null)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$tauriCli = Join-Path $root 'app\ui\node_modules\.bin\tauri.cmd'
if (-not (Test-Path $tauriCli)) { throw 'Tauri''s command line is not installed: run npm ci in app\ui first' }
$key = Join-Path $Folder 'prismqda-update.key'
if (Test-Path $key) { throw "There is already a key at $key. It is not replaced (replacing it would stop every installed program from updating). Move it away yourself if you mean to." }
if ($root -like "$Folder*" -or $Folder -like "$root*") { throw 'The key folder must not be inside the repository' }
New-Item -ItemType Directory -Force $Folder | Out-Null

if ($Password) {
    & $tauriCli signer generate --ci -w $key -p $Password
} else {
    & $tauriCli signer generate -w $key
}
if ($LASTEXITCODE -or -not (Test-Path "$key.pub")) { throw 'the key was not made' }

$public = (Get-Content "$key.pub" -Raw).Trim()
"The public key (safe to publish):"
$public
""
if ($NoSource) {
    "Not put into updates.rs (-NoSource)."
} else {
    $path = Join-Path $root 'app\src-tauri\src\updates.rs'
    $text = [System.IO.File]::ReadAllText($path)
    $pattern = 'pub const PUBKEY: &str = "[^"]*";'
    if ($text -notmatch $pattern) { throw 'PUBKEY was not found in updates.rs' }
    $replaced = [regex]::Replace($text, $pattern, ('pub const PUBKEY: &str = "' + $public + '";'))
    [System.IO.File]::WriteAllText($path, $replaced, (New-Object System.Text.UTF8Encoding $false))
    "Put into app\src-tauri\src\updates.rs. Review it with git diff, then commit it."
}
""
"Now: BACK UP $Folder (the private key and its password). Then build a release with"
"  `$env:TAURI_SIGNING_PRIVATE_KEY_PATH = '$key'"
"  `$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '<the password>'"
"  powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1 -Sign -Updates"
