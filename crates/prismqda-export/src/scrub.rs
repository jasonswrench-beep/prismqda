//! The list of names to replace, applied on the way out (decisions 138 and 156).
//!
//! While the researcher's switch is on, the caller of an export opens a scope with `scrub_scope`; every text that the Word, Excel, CSV,
//! and HTML writers put in a file passes through the list while the scope is open. The scrubbing is done where the text is written and
//! not where it is gathered, so a new kind of export cannot forget it: if it writes a file through these writers, it is scrubbed.

use std::borrow::Cow;
use std::cell::RefCell;

use prismqda_engine::Scrubber;

thread_local! {
    static CURRENT: RefCell<Option<Scrubber>> = const { RefCell::new(None) };
}

/// Closes the scope when dropped, putting back whatever was in force before.
pub struct ScrubGuard {
    previous: Option<Scrubber>,
}

impl Drop for ScrubGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT.with(|c| *c.borrow_mut() = previous);
    }
}

/// Applies `scrubber` to every text written by this crate's writers on this thread until the guard is dropped. `None` turns it off.
#[must_use = "the list stops being applied when the guard is dropped"]
pub fn scrub_scope(scrubber: Option<Scrubber>) -> ScrubGuard {
    let previous = CURRENT.with(|c| c.replace(scrubber));
    ScrubGuard { previous }
}

/// True while a list of names is being applied.
pub(crate) fn active() -> bool {
    CURRENT.with(|c| c.borrow().is_some())
}

/// The text with the list applied, if one is in force.
pub(crate) fn text(s: &str) -> Cow<'_, str> {
    CURRENT.with(|c| match &*c.borrow() {
        Some(scrubber) => match scrubber.scrub(s) {
            Cow::Borrowed(_) => Cow::Borrowed(s),
            Cow::Owned(o) => Cow::Owned(o),
        },
        None => Cow::Borrowed(s),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use prismqda_engine::DeidRule;

    fn rule() -> DeidRule {
        DeidRule { id: 1, term: "Sarah".into(), replacement: "P1".into(), kind: "pseudonym".into(), match_case: false, whole_word: true, note: String::new(), created_at: String::new() }
    }

    #[test]
    fn the_scope_applies_the_list_and_then_puts_back_what_was_there() {
        assert_eq!(text("Hi Sarah"), "Hi Sarah");
        {
            let _outer = scrub_scope(Some(Scrubber::new(&[rule()])));
            assert!(active());
            assert_eq!(text("Hi Sarah"), "Hi P1");
            {
                let _inner = scrub_scope(None);
                assert_eq!(text("Hi Sarah"), "Hi Sarah");
            }
            assert_eq!(text("Hi Sarah"), "Hi P1", "the outer scope is back");
        }
        assert!(!active());
    }
}
