//! Which paths PrismQDA may touch on the strength of what a file says.
//!
//! A recording is linked by where it is on this computer, and that place is written into the project (and into the exchange files other
//! programs write). A project or an exchange file that someone else made can therefore name any place. On Windows, merely asking whether
//! a file at `\\server\share\talk.mp3` exists makes the computer connect to `server` and offer the person's sign-in hash. So a place that
//! came from inside a file is only looked at when it is on a drive with a letter. A recording that is on a network folder is linked
//! through a letter the person has given that folder (`Z:\talk.mp3`), which they do themselves.

/// True for a path that names a place on a drive with a letter (`C:\talks\a.mp3`, `C:/talks/a.mp3`), and false for everything else:
/// a network path (`\\server\share\a.mp3`, `//server/share/a.mp3`), a verbatim or device path (`\\?\C:\a.mp3`, `\\?\UNC\server\share`,
/// `\\.\pipe\x`, `\??\C:\a.mp3`), a path with a drive letter but no folder separator after it (`C:a.mp3`, which means the current folder of
/// that drive), a path with no drive (`\a.mp3`, `a.mp3`), and an empty one. The text is looked at as it is, whatever the computer, so the
/// answer is the same in a test on any system. (On a computer that is not Windows a path that starts with one `/` is accepted instead.)
pub fn is_local_drive_path(path: &str) -> bool {
    let p = path.trim();
    let b = p.as_bytes();
    if cfg!(windows) {
        b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
    } else {
        p.starts_with('/') && !p.starts_with("//")
    }
}

#[cfg(test)]
mod tests {
    use super::is_local_drive_path as local;

    #[test]
    fn network_verbatim_and_device_paths_are_never_local() {
        for p in [
            r"\\example.invalid\s\a.mp3",
            "//example.invalid/s/a.mp3",
            r"\\?\C:\talks\a.mp3",
            r"\\?\UNC\example.invalid\s\a.mp3",
            r"\\.\pipe\x",
            r"\\.\C:\a.mp3",
            r"\??\C:\a.mp3",
            r"\\localhost\C$\talks\a.mp3",
            "//localhost/C$/talks/a.mp3",
            r"//?/C:/a.mp3",
            "  \\\\example.invalid\\s\\a.mp3",
        ] {
            assert!(!local(p), "{p} must not be local");
        }
    }

    #[test]
    fn what_has_no_drive_and_folder_is_not_local() {
        for p in ["", " ", "a.mp3", r"talks\a.mp3", r"\a.mp3", "C:", "C:a.mp3", "C", "1:\\a.mp3", "::\\a.mp3", "C;\\a.mp3"] {
            assert!(!local(p), "{p:?} must not be local");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_drive_letter_and_a_separator_is_local() {
        for p in [r"C:\talks\a.mp3", "C:/talks/a.mp3", r"d:\a.mp3", r"Z:\a.mp3", r"  C:\a.mp3  ", r"C:\"] {
            assert!(local(p), "{p} is a place on a drive");
        }
    }
}
