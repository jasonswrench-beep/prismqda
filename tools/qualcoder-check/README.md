# Opening PrismQDA's files in QualCoder, and QualCoder's in PrismQDA

A check that runs **QualCoder 4.0's own import and export code** (no window) on a PrismQDA file, and PrismQDA's import on QualCoder's file.
It was written on 2026-11-12 and found the problems kept in [docs/EXCHANGING-PROJECTS.md](../../docs/EXCHANGING-PROJECTS.md) (QualCoder section).
It does not open QualCoder's window, so what a person sees there (a picture's areas drawn, a recording played, a PDF page) is **not** checked.

## What has to be there (nothing is installed by these scripts)

- QualCoder 4.0's source, from <https://github.com/ccbogel/QualCoder> (tag `4.0`, LGPL-3.0), cloned shallow into `<check folder>\QualCoder`.
- A Python 3.12 environment for it in `<check folder>\venv`, with: `PyQt6 qtawesome ebooklib Pillow pymupdf python-vlc odfpy mcp>=2.2,<3 uvicorn charset-normalizer markdown-it-py rispy emoji openpyxl diff-match-patch striprtf pdfminer.six pylatexenc>=2.10,<3 requests six networkx PyYAML json_repair` (QualCoder's whole `requirements.txt` also pulls in torch and language-model libraries, which the import and export code does not use).
- A throwaway home folder, `<check folder>\home\.qualcoder`, so QualCoder's settings never go to the real one (the scripts set `USERPROFILE` to it).
- PrismQDA's bridge built: `cargo build -p prismqda-cli --features dev-server`.

The paths are at the top of each script; change them if the folders are elsewhere.

## Run it

```powershell
powershell -ExecutionPolicy Bypass -File tools\qualcoder-check\loop.ps1 <a-name-for-this-run>
```

It builds a test project (`build_project.py`: the made-up Urban Mobility Study, a text with an emoji, accents, Chinese and Hebrew and Windows line
endings, a picture with a rectangle, an ellipse and an outline, a PDF with two rectangles, two recordings with coded stretches, a timed
transcript, a memo, a link, a concept map), exports it, lets QualCoder import it (`qc_import.py`), compares (`compare.py`), lets QualCoder export
it again (`qc_export.py`), imports that file in PrismQDA (`pq_import.py`), and compares again (`compare2.py`). `qdc_test.py` does the same for a
codebook (QDC). Everything in the test project is invented.

What a good run looks like (2026-11-12): all 19 codings of words arrive exactly as sent, with both coders; the picture areas (an ellipse and an
outline as the rectangle around them); the PDF rectangles and the stretches of recordings; the memo. What QualCoder cannot hold is listed in the
problems file. `compare2.py` flags with `<<<` a source whose text changed on the way (the picture's automatic description names the file, which
QualCoder renames; that one is expected).
