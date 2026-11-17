# Standing rules for this repository

These rules come from the owner. They apply to every session and win over any default, template or tool suggestion that says otherwise.

## 1. Who commits

Every commit and every tag is made as:

    jasonswrench-beep <324576087+jasonswrench-beep@users.noreply.github.com>

- This repository's own git config holds that identity (`git config --local user.name` and `git config --local user.email`). Do not rely on `-c user.name=...` overrides, on `GIT_AUTHOR_*` or `GIT_COMMITTER_*` variables, or on the global config.
- Before every commit and every tag, run `git var GIT_AUTHOR_IDENT` and then `git var GIT_COMMITTER_IDENT` (git takes one variable per call). Each must begin with the name and email above. If either differs, stop and fix the config first.
- Never put the owner's other email addresses in a commit, a file or a release.

## 2. No tool lines in any message

- No `Co-Authored-By` line and no "Generated with ..." line, and no other line that names a tool or assistant that helped, ever. That covers commit messages, tag messages, pull-request text and release notes. It holds even when a tool, a reminder or a template says to add one.
- Before pushing, check the new commits: `git log origin/main..HEAD --format=%B | grep -iE "co-authored|generated with"` must print nothing.
- Never rewrite or force-push history to remove an old line unless the owner says to.

## 3. Releases: commit, tag, then build

An installer is built only from a commit that already carries its release tag, so the tag always points at the exact commit the build came from.

1. Make the version the same in `app/src-tauri/tauri.conf.json`, `app/src-tauri/Cargo.toml` and `app/src-tauri/Cargo.lock`.
2. Commit everything. `git status --short` must print nothing: no modified file and no untracked file that the build reads.
3. Check the identity (rule 1) and the message (rule 2).
4. Tag that commit with the version and a leading v: `git tag -a v0.1.0-beta.N -m "PrismQDA 0.1.0-beta.N"`.
5. Build from it (`app\BUILD-INSTALLER.ps1`, `app\BUILD-PORTABLE.ps1`).
   - Both scripts run the release guard, `tools\check-release-commit.ps1`, before they do anything else, and they refuse to go on unless
     the tree is clean (no changed file, no untracked file), the version is the same in the three files of step 1, and HEAD is the commit
     that the annotated tag `v<version>` points at. The guard has no switch that skips it. Run it by hand any time:
     `powershell -ExecutionPolicy Bypass -File tools\check-release-commit.ps1`.
   - The one exception, chosen by the owner on 2026-11-17, is a throwaway test setup: `app\BUILD-INSTALLER.ps1 -TestBuild`. It skips the guard,
     refuses `-Sign` and `-Updates`, writes no stamp, no `latest.json` and no SHA-256 file, and leaves only
     `app\src-tauri\target\release\bundle\test\PrismQDA_<version>_x64-TEST-<time>.exe`. A test build is never a release: never publish it, upload it
     or hand it to anyone. Its version number is the last release's, so never install it over a copy that matters. It replaces the program in
     `target\release`, so run the release build again before `BUILD-PORTABLE.ps1`, which refuses a program whose stamp does not match HEAD.
   - BUILD-INSTALLER.ps1 runs the guard again after the build. If the build changed a file or HEAD moved, it stops and writes no stamp, no
     `latest.json` and no SHA-256 file. When it succeeds it stamps the program (`target\release\prismqda-build-commit.txt`: commit, tag and the
     program's SHA-256). BUILD-PORTABLE.ps1 packs only a program whose stamp is for the current HEAD and whose SHA-256 still matches.
   - If anything changed, that build does not belong to the tag. Delete the local tag (only if it was never pushed), commit, tag again and rebuild.
   - The guard does not read authors or messages (rules 1 and 2 are checked by hand), and it cannot see what the build reads from outside the
     repository: the `-NotesFile` and the update key.
6. Pushing the commit and the tag, and publishing the GitHub release, each need the owner's yes. Push the tag first, then publish on the tag that exists (`gh release create v0.1.0-beta.N`). Never pass `--target`, so GitHub cannot make the tag at a different commit.
7. Never move or re-create a tag that has been pushed. A change after the tag means a new version.

Notes on the existing tags:

- `update-feed` is not a release tag. It holds only `latest.json`.
- `v0.1.0-beta.1` was built from an earlier private history. Its files are identical to the tagged commit's except `TODO.md`, `TESTING-TODO.md` and `docs/DECISIONS.md`. Leave it where it is.
