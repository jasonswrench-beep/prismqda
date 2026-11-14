# Checking a project against the REFI-QDA schema

PrismQDA writes and reads whole projects in the REFI-QDA project exchange format (`.qdpx`, [decision 159](../../docs/DECISIONS.md)).
A `.qdpx` file is a ZIP that holds `project.qde` (XML) and a flat `sources` folder. This folder keeps what is needed to check the XML
against the standard's own schema.

- `Project.xsd` is the project schema of the standard (QDA-XML 1.0, namespace `urn:QDA-XML:project:1.0`). It was copied out of section
  5.3 of the REFI-QDA 1.5 specification (the PDF), because the PDF is where this copy of the standard was read. Two things were repaired
  in copying: the page headers and page numbers that the PDF puts in the middle of the text were taken out, and a line in the pattern that
  checks a GUID, which the PDF had broken across two lines, was joined. Nothing else was changed.
- `validate-qde.ps1` validates a `project.qde` against it with the XML validator that comes with Windows PowerShell.

The file `Project.xsd` is part of the REFI-QDA specification, which is licensed under the MIT license:

> Copyright 2019 REFI-QDA (www.qdasoftware.org)
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the
> "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge,
> publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so,
> subject to the following conditions: the above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
> MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
> FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
> WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## To check an export

1. Make a `.qdpx` file (Report > Export > The whole project), or keep the one the tests make:
   `$env:PRISMQDA_KEEP_QDPX = 'C:\Temp\check.qdpx'; cargo test -p prismqda-export --test qdpx a_project_written`
2. Unzip it and take out `project.qde`.
3. `powershell -ExecutionPolicy Bypass -File tools\qdpx\validate-qde.ps1 -Xml C:\Temp\project.qde`

It prints `VALID`, or `INVALID` with the first problems. On 2026-11-10 an export of the test project was checked this way and was valid.

A file that is valid against the schema is not proved to open well in another program: that was not tried (see TESTING-TODO.md).

## A second reader

`second_reader.py` reads a `.qdpx` with **pyrefiqda** (MIT; `pip install pyrefiqda` in a throwaway environment), whose models come from the standard's own schema, and prints what it found: coders, codes, sources, selections, codings, cases, notes, links and graphs. It is a second opinion on the shape of a file (the counts can be compared with the project); it does not say how another program would show it. Run it with `python -I tools/qdpx/second_reader.py file.qdpx [folder-to-unzip-into]`.
