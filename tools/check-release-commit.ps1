# The release guard (CLAUDE.md, rule 3). An installer or a portable zip is built only from a commit that already carries its
# version tag, in a tree with nothing uncommitted, so the tag is always the exact commit the build came from.
#
#   powershell -ExecutionPolicy Bypass -File tools\check-release-commit.ps1                       by hand, before a build
#   powershell -ExecutionPolicy Bypass -File tools\check-release-commit.ps1 -ExpectCommit <sha>   after a build: nothing has moved
#
# app\BUILD-INSTALLER.ps1 runs it before it builds and again after the build; app\BUILD-PORTABLE.ps1 runs it before it packs.
# It stops (throws, so the build stops) unless all of these hold:
#   1. this folder is the top of a git checkout and `git status` shows nothing: no changed file and no untracked file
#      (files that .gitignore leaves out, such as app\src-tauri\target, do not count);
#   2. the version is the same in app\src-tauri\tauri.conf.json, app\src-tauri\Cargo.toml and app\src-tauri\Cargo.lock;
#   3. the tag v<version> exists, is an annotated tag (git tag -a), and points at HEAD;
#   4. with -ExpectCommit: HEAD is still that commit.
# On success it prints one line and returns an object with Version, Tag and Commit. It changes nothing and uses no network.
# It does not read authors or messages: the identity and the no-tool-lines rules (CLAUDE.md, rules 1 and 2) are checked by hand before
# each commit and each push. It cannot see the files a build reads from outside the repository (the notes file and the update key).
# This script has no switch that skips it. The one exception is app\BUILD-INSTALLER.ps1 -TestBuild, which does not run it and can only make an
# unsigned file named *-TEST-<time>.exe in target\release\bundle\test.

param([string]$ExpectCommit = '')

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$tauri = Join-Path $root 'app\src-tauri'

function Stop-Release([string]$Why) { throw "Release guard: $Why" }

function Invoke-Git {
    $out = & git -C $root @args
    if ($LASTEXITCODE) { Stop-Release "git $($args -join ' ') failed (exit code $LASTEXITCODE)" }
    $out
}

if (-not (Get-Command git -ErrorAction SilentlyContinue)) { Stop-Release 'git was not found, so the build cannot be tied to a commit' }

# 1. A git checkout, nothing uncommitted.
$inside = & git -C $root rev-parse --is-inside-work-tree
if ($LASTEXITCODE -or "$inside".Trim() -ne 'true') { Stop-Release "$root is not a git checkout, so the build cannot be tied to a commit" }
$prefix = "$(Invoke-Git rev-parse --show-prefix)".Trim()
if ($prefix) { Stop-Release "$root is inside a larger git checkout ($prefix), not the top of its own" }

$dirty = @(Invoke-Git status --porcelain --untracked-files=all)
if ($dirty.Count) {
    $listed = ($dirty | Select-Object -First 10 | ForEach-Object { "  $_" }) -join "`n"
    $more = if ($dirty.Count -gt 10) { "`n  ... and $($dirty.Count - 10) more" } else { '' }
    Stop-Release "the tree is not clean ($($dirty.Count) changed or untracked). Commit everything, then tag the commit, then build.`n$listed$more"
}

# 2. One version in the three places.
$confVersion = [string](Get-Content (Join-Path $tauri 'tauri.conf.json') -Raw | ConvertFrom-Json).version
$cargoToml = Get-Content (Join-Path $tauri 'Cargo.toml') -Raw
if ($cargoToml -notmatch '(?ms)^\[package\]\s.*?^version\s*=\s*"([^"]+)"') { Stop-Release 'no version was found in app\src-tauri\Cargo.toml' }
$tomlVersion = $Matches[1]
$cargoLock = Get-Content (Join-Path $tauri 'Cargo.lock') -Raw
if ($cargoLock -notmatch '(?m)^name = "prismqda-app"\r?\nversion = "([^"]+)"') { Stop-Release 'prismqda-app was not found in app\src-tauri\Cargo.lock' }
$lockVersion = $Matches[1]
if ($confVersion -ne $tomlVersion -or $confVersion -ne $lockVersion) {
    Stop-Release "the version is not the same everywhere: tauri.conf.json $confVersion, Cargo.toml $tomlVersion, Cargo.lock $lockVersion"
}

# 3. The tag, on this commit.
$tag = "v$confVersion"
$tagRef = "refs/tags/$tag"
$head = "$(Invoke-Git rev-parse HEAD)".Trim()
$short = $head.Substring(0, 7)
$null = & git -C $root rev-parse -q --verify $tagRef
if ($LASTEXITCODE) {
    $carried = @(Invoke-Git tag --points-at HEAD)
    $carriedText = if ($carried.Count) { "HEAD ($short) carries: $($carried -join ', ')" } else { "HEAD ($short) carries no tag" }
    Stop-Release "the tag $tag does not exist. $carriedText. Tag the commit first:  git tag -a $tag -m `"PrismQDA $confVersion`""
}
$tagKind = "$(Invoke-Git cat-file -t $tagRef)".Trim()
if ($tagKind -ne 'tag') { Stop-Release "$tag is a lightweight tag. Make it an annotated one on this commit (git tag -d $tag, then git tag -a $tag -m `"PrismQDA $confVersion`"); only a tag that was never pushed may be re-made" }
$tagged = "$(Invoke-Git rev-parse "$tagRef^{commit}")".Trim()
if ($tagged -ne $head) {
    Stop-Release "$tag points at $($tagged.Substring(0, 7)) but HEAD is $short, so a build from here would not be the tagged commit. A tag that was never pushed may be re-made on HEAD; a pushed tag never moves, so a change after it needs a new version and a new tag"
}

# 4. After a build: nothing moved.
if ($ExpectCommit -and $head -ne $ExpectCommit) { Stop-Release "HEAD was $($ExpectCommit.Substring(0, 7)) when the build started and is $short now" }

Write-Host "Release guard: clean tree, HEAD $short carries $tag (version $confVersion)."
[pscustomobject]@{ Version = $confVersion; Tag = $tag; Commit = $head }
