# Double-clicking a project: the installed program, in its real window

`run-real-launch.ps1` checks the installed PrismQDA the way a person double-clicking a `.pqda` file in Windows Explorer would meet it
(decision 178). It drives the **real window** (WebView2 inside the Tauri shell) over WebView2's debugging port, so it reads the page and
sends no operating-system mouse or keyboard events.

```powershell
powershell -ExecutionPolicy Bypass -File tools\installer-check\run-real-launch.ps1 -Installer <path to PrismQDA_<version>_x64-setup.exe>
```

- Leave out `-Installer` to check the copy that is already installed (`%LOCALAPPDATA%\PrismQDA`). With it, the installer is run silently
  (`/S`) over the installed copy first, as a person's upgrade would.
- It **stops** unless the computer has been idle for ten minutes (it opens PrismQDA's window) **or the person at the computer has said to go ahead** (set `PRISMQDA_IDLE_OK=1`; they then keep their hands off the keyboard and mouse), and stops if a PrismQDA is already running
  (a second start would be handed to that window).
- The window's own data goes to a scratch folder (`WEBVIEW2_USER_DATA_FOLDER`), so the person's settings and recent list are not touched.

What it looks at: `.pqda` is registered to the installed program for the current user; a double click opens the project; a second start
hands its file to the window already open and ends (one program, not two); the project already open is not opened again; an encrypted
project asks for its passphrase; a file that only has the `.pqda` ending is said in words and leaves the open project as it was; the
window closes and leaves no `-wal` file with work in it.

It needs Node.js and Playwright (the folder named by `PLAYWRIGHT_DIR`, by default `playwright-tools\node_modules\playwright` in the user's profile folder), and the development bridge
built with `cargo build -p prismqda-cli --features dev-server`, which `make_projects.mjs` uses to make the made-up projects.

What it does **not** show: Explorer's own double click (the file type is started with `Start-Process`, which asks the shell the same
question), the file's icon, the *Open with* list, a second computer, Windows 10, and anything with the portable copy (it registers nothing
and keeps to itself). Those are in [TESTING-TODO.md](../../TESTING-TODO.md).

## The recording tools, in the real window

`run-real-recording.ps1` opens the installed program on a project the check makes (`make_recording_project.mjs`: a recording of eleven
minutes and 5.2 MB, larger than the 4 MiB a window is sent at once, with one loud second at ten minutes and a coded stretch on it) and
`recording_probe.mjs` tries the recording tools in the window over the debugging port (9336): the waveform is drawn from the whole recording
(the window reads it through its own address, `http://media.localhost`, under the content security policy), the loud second is found, and
the coded stretch is saved as a WAV file whose header, length and loudness are read back; the console is searched for policy and
cross-origin errors. **It plays nothing**: the waveform and the clip are decoded, never played, so the Clips player is not tried here. It
stops unless the computer has been idle for ten minutes and when a PrismQDA is already running, like the other checks.

## A signed setup

`run-real-launch.ps1 -Installer <the signed setup>` is the check to run on a setup made with `app\BUILD-INSTALLER.ps1 -Sign`: it installs it silently over the installed copy and then drives the real window as above. Afterwards `python tools\check_signatures.py --installed` checks the installed program and `uninstall.exe` (an installed program is signed and timestamped, or it says what is not).
