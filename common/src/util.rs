pub fn strip_lead_char(string: &str, character: char, multiple: bool) -> &str {
    if string.len() == 0 {
        return string;
    }

    let bytes: &[u8] = string.as_bytes();
    let mut i: usize = 0usize;

    // NOTE: setting var i to the new string slice head
    loop {
        if bytes[i] as char == character {
            i = i + 1usize;

            if i >= bytes.len() || !multiple {
                break;
            }
        } else {
            break;
        }
    }

    &string[i..]
}
