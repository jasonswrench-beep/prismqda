# Icon and logo design notes

The artwork itself is in [../../assets](../../assets): `brand/` (logo, wordmark, app icon, and the two file glyphs for a project and an exchange package), `icons/regular/` (227 outline icons from the three batches) and `icons/active/` (12 filled, selected-tab icons). This folder keeps the notes that came with it.

| File | What it is |
| --- | --- |
| [Icon_Standards_v0.1.md](Icon_Standards_v0.1.md) | The rules every icon follows (24 pixel grid, 1.75 pixel stroke, round caps, one color) |
| [Design_Decisions.md](Design_Decisions.md) | The drawing decisions and which are still waiting for approval |
| [Review_Checklist.md](Review_Checklist.md) | Ten questions to answer at actual size before any icon is called approved |
| [Integration_Guide.md](Integration_Guide.md) | How the pack suggests using the files. The interface here does it through a generated module instead (decision 41) |
| [Validation_Report.md](Validation_Report.md) | What the pack's own technical checks covered, and what they did not |
| [review-gallery.html](review-gallery.html) | An offline gallery of the Batch 1 icons at 16, 20, 24, and 68 pixels, in light, dark, and high contrast. It reads the files in `assets/`, so open it from this folder |
| [review-gallery-batch2.html](review-gallery-batch2.html) | The same kind of gallery for the 122 Batch 2 files, with search, family filters, and review notes you can export |
| [review-gallery-batch3.html](review-gallery-batch3.html) | The gallery for the last 14 icons |
| [batch3](batch3) | The Batch 3 notes: readme, changelog, design decisions, and validation report |
| [batch2](batch2) | The Batch 2 notes: its readme, changelog, design decisions (B2-01 to B2-14), icon standard additions, provenance, and validation report |
| [registry](registry) | All 244 planned icons with their batch and status (`full-library-registry`), and the Batch 2 manifest. The `path` fields keep the packs' folder names (`icons/outline`, `brand`); here those are `assets/icons/regular` and `assets/brand` |
| [source](source) | The icon brief as supplied |
| [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt) | Where the wordmark letters come from (Inter, open font license) |
| [PACK-README.md](PACK-README.md), [Release_Notes.md](Release_Notes.md), [Batch1_Completion_Notes.md](Batch1_Completion_Notes.md) | The pack's own notes |

Status: the logo direction is approved. Every icon is a draft that nobody has reviewed at actual size. All 244 planned icons are drawn, in three batches (108, 122, and 14). The 244 are the 227 outline icons, the 12 selected-tab icons, the three core brand files (`brand-mark.svg`, `brand-logo.svg`, `app-icon.svg`), and the two file glyphs. Two optional brand versions (`brand-logo-light.svg`, `brand-mark-small.svg`) and `app-icon.ico` are in the repository but are not counted in the 244.

The two file glyphs in `assets/brand` (`file-project.svg`, `file-package.svg`) carry the prism's colors, so they are images and not part of the themeable set. Five of the Batch 2 icons use a dashed line; the generator allows a dash pattern written as numbers and nothing else.

To change an icon, edit its SVG file in `assets/icons`, then run `npm run icons` in `app/ui`. The generator refuses an icon that breaks the standard, and the unit tests fail until the generated module matches the files.

The pack's own checking script was not copied: it expects the pack's folder layout, and the generator and `app/ui/src/lib/icons.test.ts` check the same rules in place.
