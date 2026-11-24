# Test fixtures for the updater (`src/updates.rs`)

These files are **test-only**. They have nothing to do with the key that signs PrismQDA's releases.

| File | What it is |
| --- | --- |
| `update-setup.bin` | 284 arbitrary bytes standing in for a setup file. Every signature below is of these bytes. |
| `update-test.pub` | Public half of a throwaway "main" key (what `PUBKEY` would be). |
| `update-backup.pub` | Public half of a throwaway "backup" key (what `BACKUP_PUBKEY` would be). |
| `update-other.pub` | Public half of a third throwaway key that neither of the others trusts. |
| `update-setup.bin.sig` | Signed by the main key, as the file `PrismQDA_9.9.9_x64-setup.exe` (the name is in the signature's trusted comment). |
| `update-setup-old.bin.sig` | Signed by the main key, as `PrismQDA_0.1.0-beta.3_x64-setup.exe`: a genuine signature of an older installer. |
| `update-setup-backup.bin.sig` | Signed by the backup key, as `PrismQDA_9.9.9_x64-setup.exe`. |
| `update-setup-third.bin.sig` | Signed by the third key, as `PrismQDA_9.9.9_x64-setup.exe`. |

The private halves of the three keys were made with `tauri signer generate` (password `test`), used once to sign, and deleted. Only the public halves and the
signatures are kept, so the fixtures cannot be extended without making new keys: to change one, make three new throwaway keys, sign `update-setup.bin` again
under each file name above (copy it to a file of that name, then `tauri signer sign -f <key> <file>`), replace all the `.pub` and `.sig` files together, and run
`cargo test` in `app/src-tauri`. Never put a real update key, or its password, anywhere in this folder.
