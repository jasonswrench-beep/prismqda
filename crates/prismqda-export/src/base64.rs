//! Base 64, the way a picture travels from the interface to the engine and into a web page.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The text of `bytes`, with padding and no line breaks.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

/// The bytes of `text`. White space is ignored and the padding is optional; any other character that is not in the alphabet, or a
/// length that cannot be right, gives `None`.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let mut values: Vec<u8> = Vec::with_capacity(text.len());
    let mut padding = 0;
    for c in text.bytes() {
        match c {
            b' ' | b'\t' | b'\r' | b'\n' => {}
            b'=' => padding += 1,
            _ if padding > 0 => return None,
            b'A'..=b'Z' => values.push(c - b'A'),
            b'a'..=b'z' => values.push(c - b'a' + 26),
            b'0'..=b'9' => values.push(c - b'0' + 52),
            b'+' => values.push(62),
            b'/' => values.push(63),
            _ => return None,
        }
    }
    if padding > 2 || values.len() % 4 == 1 || (padding > 0 && !(values.len() + padding).is_multiple_of(4)) {
        return None;
    }
    let mut out = Vec::with_capacity(values.len() / 4 * 3 + 2);
    for chunk in values.chunks(4) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, v)| acc | (u32::from(*v) << (18 - 6 * i)));
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_examples_encode_and_decode() {
        for (plain, coded) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")] {
            assert_eq!(encode(plain.as_bytes()), coded);
            assert_eq!(decode(coded).unwrap(), plain.as_bytes());
        }
    }

    #[test]
    fn every_byte_value_survives_a_round_trip() {
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(decode(&encode(&all)).unwrap(), all);
        assert_eq!(decode(&encode(&all[..254])).unwrap(), &all[..254]);
    }

    #[test]
    fn white_space_is_ignored_and_padding_is_optional() {
        assert_eq!(decode("Zm9v\r\nYmFy").unwrap(), b"foobar");
        assert_eq!(decode("Zm9vYg").unwrap(), b"foob");
        assert_eq!(decode("Zm9vYmE").unwrap(), b"fooba");
    }

    #[test]
    fn text_that_is_not_base_64_is_refused() {
        assert!(decode("Zm9v!").is_none());
        assert!(decode("Z").is_none(), "one character cannot hold a byte");
        assert!(decode("Zg=a").is_none(), "nothing follows the padding");
        assert!(decode("Zg===").is_none(), "too much padding");
        assert!(decode("Zg=").is_none(), "padding that does not complete the group");
    }
}
