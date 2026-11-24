# Checks the update keys before app\BUILD-INSTALLER.ps1 signs a release for the Check for updates button (docs\UPDATES.md). It stops (throws) if
# the key about to sign is not the one the program built from this source will trust. It reads only the key's public half (the .pub file beside
# the private key file); it never opens the private key, and it changes nothing.
#
#   powershell -ExecutionPolicy Bypass -File tools\check-update-keys.ps1 -KeyFile <the .key file>                                 a normal release
#   powershell -ExecutionPolicy Bypass -File tools\check-update-keys.ps1 -KeyFile <the BACKUP .key file> -Rotation -PreviousTag v0.1.0-beta.4   a rotation
#
# A normal release: PUBKEY in updates.rs must be the public half of the key, and BACKUP_PUBKEY (if set) must be a different key. With no backup key
# built in it warns, because a lost main key would then strand every installed program for good.
#
# A rotation release (docs\UPDATES.md, "Rotating the keys") is signed with the BACKUP key, because that is the one key the installed programs still
# trust when the main key is lost or stolen. So: the key about to sign must be exactly the BACKUP_PUBKEY of the release named by -PreviousTag (read from
# git, not from the working copy), and the source must bring in a new PUBKEY and a new BACKUP_PUBKEY, neither of them the key that signs it.

param(
    [Parameter(Mandatory = $true)][string]$KeyFile,
    [string]$SourceFile = '',
    [string]$Repo = '',
    [switch]$Rotation,
    [string]$PreviousTag = ''
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $SourceFile) { $SourceFile = Join-Path $root 'app\src-tauri\src\updates.rs' }
if (-not $Repo) { $Repo = $root }

function Get-Constant([string]$Text, [string]$Name) {
    if ($Text -notmatch ('pub const ' + $Name + ': &str = "([^"]*)";')) { throw "$Name was not found in updates.rs" }
    return $Matches[1].Trim()
}
# The key id inside a public key's text, for saying which key is meant.
function Get-KeyId([string]$Public) {
    if (-not $Public) { return '(none)' }
    try {
        $first = ([System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Public)) -split "`n")[0].Trim()
        return ($first -split ' ')[-1]
    } catch { throw 'a public key is not the base64 text of a minisign public key' }
}

if (-not (Test-Path $KeyFile)) { throw 'the update key file was not found (TAURI_SIGNING_PRIVATE_KEY_PATH)' }
if (-not (Test-Path "$KeyFile.pub")) { throw "the public half of the update key is not beside it ($KeyFile.pub)" }
$signing = (Get-Content "$KeyFile.pub" -Raw).Trim()
$text = [System.IO.File]::ReadAllText($SourceFile)
$main = Get-Constant $text 'PUBKEY'
$backup = Get-Constant $text 'BACKUP_PUBKEY'
if (-not $main) { throw 'PUBKEY in app\src-tauri\src\updates.rs is empty, so the program would not check for updates at all (tools\make-update-key.ps1 puts it there)' }
if ($backup -and $backup -eq $main) { throw 'BACKUP_PUBKEY is the same key as PUBKEY: a backup that is the main key protects nothing' }

if ($Rotation) {
    if (-not $PreviousTag) { throw '-Rotation needs -PreviousTag: the last release that people have installed' }
    $previous = & git -C $Repo show "${PreviousTag}:app/src-tauri/src/updates.rs"
    if ($LASTEXITCODE) { throw "could not read updates.rs of $PreviousTag from git" }
    $previousBackup = Get-Constant ($previous -join "`n") 'BACKUP_PUBKEY'
    if (-not $previousBackup) { throw "$PreviousTag has no backup key built in, so no release can rotate its keys; its users install a new setup file by hand once" }
    if ($signing -ne $previousBackup) {
        throw "the key about to sign (id $(Get-KeyId $signing)) is not the backup key that $PreviousTag trusts (id $(Get-KeyId $previousBackup)): programs installed from $PreviousTag would refuse this release"
    }
    if (-not $backup) { throw 'a rotation release must bring in a new BACKUP_PUBKEY, or the next loss of a key could not be recovered from' }
    if ($main -eq $signing -or $backup -eq $signing) { throw 'the new PUBKEY or BACKUP_PUBKEY is still the key that signs this release: a rotation must bring in new keys' }
    "Rotation release: signed with the backup key of $PreviousTag (id $(Get-KeyId $signing)); it brings in the main key $(Get-KeyId $main) and the backup key $(Get-KeyId $backup)."
    "Afterwards the key $(Get-KeyId $signing) is retired: keep it offline, and never sign another release with it."
} else {
    if ($backup -and $signing -eq $backup) { throw 'this is the BACKUP key. It signs only a rotation release (-Rotation -PreviousTag ...), never a normal one' }
    if ($signing -ne $main) { throw 'PUBKEY in updates.rs is not the public half of the update key; a program built like that would refuse every update' }
    if (-not $backup) {
        Write-Warning 'No backup update key is built in (BACKUP_PUBKEY is empty). If the update key or its password is lost, installed programs can never be updated again. See docs\UPDATES.md, "Rotating the keys".'
    } else {
        "Update keys: main $(Get-KeyId $main) signs this release; backup $(Get-KeyId $backup) is built in."
    }
}
