pub fn rm_lead_char(s: &str, chr: char, multiple: bool) -> &str {
    if s.len() == 0 {
        return s;
    }

    let bytes: &[u8] = s.as_bytes();
    let mut i: usize = 0usize;

    loop {
        if bytes[i] as char == chr {
            i = i + 1usize;

            if i >= bytes.len() || !multiple {
                break;
            }
        } else {
            break;
        }
    }

    &s[i..]
}
