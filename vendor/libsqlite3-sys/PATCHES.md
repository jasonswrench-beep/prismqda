# libsqlite3-sys 0.38.2, with one change

This is the `libsqlite3-sys` crate (the C bindings `rusqlite` uses), version 0.38.2, by the rusqlite developers, MIT licensed
(see `LICENSE`). Cargo uses this copy instead of the one on crates.io through the `[patch.crates-io]` table of the workspace's
`Cargo.toml` and of `app/src-tauri/Cargo.toml`.

## What is changed

Only `build.rs` and `Cargo.toml`, and only so that the bundled SQLite is the **SQLite3 Multiple Ciphers** source
(<https://github.com/utelle/SQLite3MultipleCiphers>, MIT) instead of the plain SQLite source:

* `Cargo.toml`: a build dependency on the `sqlite3mc-src` crate, pinned to `=206.1.0` (SQLite3 Multiple Ciphers 2.6.1 on SQLite
  3.54.0, shipped byte for byte as released; its checksum comes from the release's signed `SHA256SUMS`).
* `build.rs`: the `bundled` build compiles `sqlite3mc_amalgamation.c` from that crate where it compiled `sqlite3/sqlite3.c`, and the
  two `rerun-if-changed` lines for files that are no longer here are gone.

The big SQLite files of the upstream crate (`sqlite3/sqlite3.c`, `sqlite3.h`, the `sqlcipher/` folder) are left out, because
nothing uses them now. The Rust bindings in `sqlite3/bindgen_bundled_version*.rs` and everything in `src/` are as published.

## Why

A project can be encrypted (decision 157). SQLite3 Multiple Ciphers is SQLite with page encryption built in. It needs no OpenSSL and
no Perl, so it builds on a plain Windows machine with the Visual Studio build tools. PrismQDA uses its SQLCipher 4 compatible
scheme (`PRAGMA cipher = 'sqlcipher'; PRAGMA legacy = 4;`): AES-256-CBC with an HMAC-SHA512 on each page, and the key made from the
passphrase with PBKDF2-HMAC-SHA512 at 256,000 rounds. A project that is not encrypted is an ordinary SQLite file, as before.

## Upgrading

When `rusqlite` moves to a newer `libsqlite3-sys`, copy that version here, make the same two changes, and check that
`cargo test --workspace` still passes (the tests include opening and writing an encrypted project).

## Waiting for 2.7.0

SQLite3 Multiple Ciphers 2.7.0 was released on 2026-11-13 (a fix for a bug in its backup, and a rework of key changes). The `sqlite3mc-src` crate has only 2.6.1 (`=206.1.0`) on that day, so this copy stays on 2.6.1 until the crate for 2.7.0 is published: then change the pin in `Cargo.toml`, check the checksum the way it is described above, run the whole set (the encrypted-project tests included), and say so here. In the meantime every backup is opened again with its key and checked after it is made (`backup_to` in the engine's `project.rs`).
