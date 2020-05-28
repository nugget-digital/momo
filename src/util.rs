pub fn rm_lead_char_plus(s: &str, chr: char) -> &str {
    let bytes: &[u8] = s.as_bytes();
    let mut i: usize = 0usize;

    loop {
        if bytes[i] as char == chr {
            i = i + 1usize;
        } else {
            break;
        }
    }

    &s[i..]
}
