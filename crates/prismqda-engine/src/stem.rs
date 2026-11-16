//! English stems (decision 202), so "connect", "connected", "connecting" and "connection" can be counted as one word. This is the algorithm of
//! M. F. Porter, "An algorithm for suffix stripping", Program 14(3), 1980: a fixed list of rules that strip endings from lower-case English
//! words, with no dictionary. It is written here from the paper's description. A stem is not always a word ("happy" is "happi"), because it is
//! only a key for gathering forms of one word; the window shows the commonest real form of each group.
//!
//! Only words made of the letters a to z are stemmed. A word with a digit, a hyphen, or a letter with an accent is left as it is, because the
//! rules know English spelling and nothing else.

struct Stemmer {
    b: Vec<u8>,
}

impl Stemmer {
    /// True when the letter at `i` is a consonant: any letter but a, e, i, o and u, and a y that follows a vowel (or starts the word).
    fn cons(&self, i: usize) -> bool {
        match self.b[i] {
            b'a' | b'e' | b'i' | b'o' | b'u' => false,
            b'y' => i == 0 || !self.cons(i - 1),
            _ => true,
        }
    }

    /// The measure of the first `len` letters: how many times a run of vowels is followed by a run of consonants.
    fn m(&self, len: usize) -> usize {
        let (mut n, mut i) = (0, 0);
        while i < len && self.cons(i) {
            i += 1;
        }
        loop {
            while i < len && !self.cons(i) {
                i += 1;
            }
            if i >= len {
                return n;
            }
            while i < len && self.cons(i) {
                i += 1;
            }
            n += 1;
        }
    }

    fn has_vowel(&self, len: usize) -> bool {
        (0..len).any(|i| !self.cons(i))
    }

    /// The first `len` letters end in a double consonant.
    fn double_consonant(&self, len: usize) -> bool {
        len >= 2 && self.b[len - 1] == self.b[len - 2] && self.cons(len - 1)
    }

    /// The first `len` letters end consonant, vowel, consonant, where the last consonant is not w, x or y.
    fn cvc(&self, len: usize) -> bool {
        len >= 3 && self.cons(len - 1) && !self.cons(len - 2) && self.cons(len - 3) && !matches!(self.b[len - 1], b'w' | b'x' | b'y')
    }

    fn ends(&self, suffix: &str) -> bool {
        self.b.ends_with(suffix.as_bytes())
    }

    /// Replaces the last `cut` letters with `with`.
    fn replace_end(&mut self, cut: usize, with: &str) {
        let keep = self.b.len() - cut;
        self.b.truncate(keep);
        self.b.extend_from_slice(with.as_bytes());
    }

    fn step1a(&mut self) {
        if self.ends("sses") {
            self.replace_end(4, "ss");
        } else if self.ends("ies") {
            self.replace_end(3, "i");
        } else if self.ends("ss") {
            // stays
        } else if self.ends("s") {
            self.replace_end(1, "");
        }
    }

    fn step1b(&mut self) {
        if self.ends("eed") {
            if self.m(self.b.len() - 3) > 0 {
                self.replace_end(1, "");
            }
            return;
        }
        let cut = if self.ends("ed") {
            2
        } else if self.ends("ing") {
            3
        } else {
            return;
        };
        if !self.has_vowel(self.b.len() - cut) {
            return;
        }
        self.replace_end(cut, "");
        let len = self.b.len();
        if self.ends("at") {
            self.replace_end(2, "ate");
        } else if self.ends("bl") {
            self.replace_end(2, "ble");
        } else if self.ends("iz") {
            self.replace_end(2, "ize");
        } else if self.double_consonant(len) && !matches!(self.b[len - 1], b'l' | b's' | b'z') {
            self.replace_end(1, "");
        } else if self.m(len) == 1 && self.cvc(len) {
            self.replace_end(0, "e");
        }
    }

    fn step1c(&mut self) {
        if self.ends("y") && self.has_vowel(self.b.len() - 1) {
            self.replace_end(1, "i");
        }
    }

    /// The first rule of the list whose ending the word has decides: the ending is replaced when what is left has a measure above `min`, and no
    /// later rule is tried either way.
    fn rules(&mut self, list: &[(&str, &str)], min: usize) {
        for (suffix, with) in list {
            if self.ends(suffix) {
                if self.m(self.b.len() - suffix.len()) > min {
                    self.replace_end(suffix.len(), with);
                }
                return;
            }
        }
    }

    fn step2(&mut self) {
        self.rules(
            &[
                ("ational", "ate"),
                ("tional", "tion"),
                ("enci", "ence"),
                ("anci", "ance"),
                ("izer", "ize"),
                ("abli", "able"),
                ("alli", "al"),
                ("entli", "ent"),
                ("eli", "e"),
                ("ousli", "ous"),
                ("ization", "ize"),
                ("ation", "ate"),
                ("ator", "ate"),
                ("alism", "al"),
                ("iveness", "ive"),
                ("fulness", "ful"),
                ("ousness", "ous"),
                ("aliti", "al"),
                ("iviti", "ive"),
                ("biliti", "ble"),
            ],
            0,
        );
    }

    fn step3(&mut self) {
        self.rules(&[("icate", "ic"), ("ative", ""), ("alize", "al"), ("iciti", "ic"), ("ical", "ic"), ("ful", ""), ("ness", "")], 0);
    }

    fn step4(&mut self) {
        // "ion" is taken off only after s or t.
        if self.ends("ion") {
            let stem = self.b.len() - 3;
            if stem > 0 && matches!(self.b[stem - 1], b's' | b't') {
                if self.m(stem) > 1 {
                    self.replace_end(3, "");
                }
                return;
            }
        }
        self.rules(
            &[
                ("al", ""),
                ("ance", ""),
                ("ence", ""),
                ("er", ""),
                ("ic", ""),
                ("able", ""),
                ("ible", ""),
                ("ant", ""),
                ("ement", ""),
                ("ment", ""),
                ("ent", ""),
                ("ou", ""),
                ("ism", ""),
                ("ate", ""),
                ("iti", ""),
                ("ous", ""),
                ("ive", ""),
                ("ize", ""),
            ],
            1,
        );
    }

    fn step5(&mut self) {
        if self.ends("e") {
            let stem = self.b.len() - 1;
            let m = self.m(stem);
            if m > 1 || (m == 1 && !self.cvc(stem)) {
                self.replace_end(1, "");
            }
        }
        let len = self.b.len();
        if self.ends("ll") && self.m(len) > 1 {
            self.replace_end(1, "");
        }
    }
}

/// The stem of an English word written in lower case; a word the rules do not apply to is returned as it is.
pub fn stem(word: &str) -> String {
    let w = word.strip_suffix("'s").unwrap_or(word);
    if w.len() < 3 || !w.bytes().all(|b| b.is_ascii_lowercase()) {
        return word.to_string();
    }
    let mut s = Stemmer { b: w.as_bytes().to_vec() };
    s.step1a();
    s.step1b();
    s.step1c();
    s.step2();
    s.step3();
    s.step4();
    s.step5();
    String::from_utf8(s.b).unwrap_or_else(|_| word.to_string())
}

#[cfg(test)]
mod tests {
    use super::stem;

    #[test]
    fn the_words_of_the_papers_examples_come_out_as_the_paper_says() {
        let cases = [
            // step 1a
            ("caresses", "caress"),
            ("ponies", "poni"),
            ("ties", "ti"),
            ("caress", "caress"),
            ("cats", "cat"),
            // step 1b
            ("plastered", "plaster"),
            ("bled", "bled"),
            ("motoring", "motor"),
            ("sing", "sing"),
            ("hopping", "hop"),
            ("falling", "fall"),
            ("hissing", "hiss"),
            ("filing", "file"),
            ("failing", "fail"),
            // step 1c
            ("happy", "happi"),
            ("sky", "sky"),
            // steps 2 and 3
            ("relational", "relat"),
            ("conditional", "condit"),
            ("rational", "ration"),
            // step 4
            ("adjustment", "adjust"),
            ("replacement", "replac"),
            ("allowance", "allow"),
            ("inference", "infer"),
            ("effective", "effect"),
            // step 5
            ("probate", "probat"),
            ("rate", "rate"),
            ("cease", "ceas"),
            ("controlling", "control"),
            ("rolling", "roll"),
            // a family of words that is one word
            ("connect", "connect"),
            ("connected", "connect"),
            ("connecting", "connect"),
            ("connection", "connect"),
            ("connections", "connect"),
        ];
        for (word, want) in cases {
            assert_eq!(stem(word), want, "{word}");
        }
    }

    #[test]
    fn a_word_the_rules_do_not_know_is_left_as_it_is() {
        for word in ["it", "a", "2024", "well-being", "café", "don't", "x1"] {
            assert_eq!(stem(word), word, "{word}");
        }
        assert_eq!(stem("bus's"), "bu", "a possessive loses its 's before the rules are applied");
    }
}
