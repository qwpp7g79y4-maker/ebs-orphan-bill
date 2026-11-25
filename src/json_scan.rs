use std::collections::HashSet;

pub(crate) enum KeyFault {
    Duplicate,
    Bad,
    /// More JSON values and object keys than [`MAX_JSON_ITEMS`].
    TooMany,
}

/// Most JSON values plus object keys one document may hold. The parsed tree costs
/// about 100 to 300 bytes for each of them, so this bounds the memory of a small
/// but dense file (nested arrays, a long list of numbers) to a few GB, where the size
/// limit of 256 MiB alone allowed tens of GB.
pub(crate) const MAX_JSON_ITEMS: u64 = 16_777_216;

/// Counts the values and keys seen so far against a maximum.
struct Budget {
    used: u64,
    max: u64,
}

impl Budget {
    fn take(&mut self) -> Result<(), KeyFault> {
        self.used += 1;
        if self.used > self.max {
            return Err(KeyFault::TooMany);
        }
        Ok(())
    }
}

/// Top-level keys of one JSON object. A repeated key is [`KeyFault::Duplicate`].
/// A value that does not start with `{` is not a duplicate-key problem.
pub(crate) fn top_level_keys(text: &str) -> Result<Vec<String>, KeyFault> {
    top_level_keys_within(text, MAX_JSON_ITEMS)
}

fn top_level_keys_within(text: &str, max_items: u64) -> Result<Vec<String>, KeyFault> {
    let bytes = text.as_bytes();
    let mut budget = Budget {
        used: 0,
        max: max_items,
    };
    let mut i = 0;
    skip_ws(bytes, &mut i);
    if i >= bytes.len() {
        return Err(KeyFault::Bad);
    }
    if bytes[i] != b'{' {
        skip_value(bytes, &mut i, 1, &mut budget)?;
        return Ok(Vec::new());
    }
    budget.take()?;
    i += 1;
    let mut keys = Vec::new();
    let mut seen = HashSet::new();
    loop {
        skip_ws(bytes, &mut i);
        if i < bytes.len() && bytes[i] == b'}' {
            return Ok(keys);
        }
        budget.take()?;
        let key = parse_json_string(bytes, &mut i)?;
        if !seen.insert(key.clone()) {
            return Err(KeyFault::Duplicate);
        }
        keys.push(key);
        skip_ws(bytes, &mut i);
        if i >= bytes.len() || bytes[i] != b':' {
            return Err(KeyFault::Bad);
        }
        i += 1;
        skip_value(bytes, &mut i, 1, &mut budget)?;
        skip_ws(bytes, &mut i);
        if i < bytes.len() && bytes[i] == b',' {
            i += 1;
            continue;
        }
        if i < bytes.len() && bytes[i] == b'}' {
            return Ok(keys);
        }
        return Err(KeyFault::Bad);
    }
}

fn skip_ws(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

fn skip_value(
    bytes: &[u8],
    i: &mut usize,
    depth: u32,
    budget: &mut Budget,
) -> Result<(), KeyFault> {
    if depth > 128 {
        return Err(KeyFault::Bad);
    }
    budget.take()?;
    skip_ws(bytes, i);
    if *i >= bytes.len() {
        return Err(KeyFault::Bad);
    }
    match bytes[*i] {
        b'"' => {
            parse_json_string(bytes, i)?;
            Ok(())
        }
        b'{' => skip_container(bytes, i, b'{', b'}', depth, budget),
        b'[' => skip_container(bytes, i, b'[', b']', depth, budget),
        b't' => skip_lit(bytes, i, b"true"),
        b'f' => skip_lit(bytes, i, b"false"),
        b'n' => skip_lit(bytes, i, b"null"),
        b'-' | b'0'..=b'9' => skip_number(bytes, i),
        _ => Err(KeyFault::Bad),
    }
}

fn skip_container(
    bytes: &[u8],
    i: &mut usize,
    open: u8,
    close: u8,
    depth: u32,
    budget: &mut Budget,
) -> Result<(), KeyFault> {
    if bytes[*i] != open {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    let mut seen = HashSet::new();
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == close {
            *i += 1;
            return Ok(());
        }
        if open == b'{' {
            budget.take()?;
            let key = parse_json_string(bytes, i)?;
            if !seen.insert(key) {
                return Err(KeyFault::Duplicate);
            }
            skip_ws(bytes, i);
            if *i >= bytes.len() || bytes[*i] != b':' {
                return Err(KeyFault::Bad);
            }
            *i += 1;
        }
        skip_value(bytes, i, depth + 1, budget)?;
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == close {
            *i += 1;
            return Ok(());
        }
        return Err(KeyFault::Bad);
    }
}

fn skip_lit(bytes: &[u8], i: &mut usize, lit: &[u8]) -> Result<(), KeyFault> {
    if bytes[*i..].starts_with(lit) {
        *i += lit.len();
        Ok(())
    } else {
        Err(KeyFault::Bad)
    }
}

fn skip_number(bytes: &[u8], i: &mut usize) -> Result<(), KeyFault> {
    let start = *i;
    if bytes[*i] == b'-' {
        *i += 1;
    }
    if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
        return Err(KeyFault::Bad);
    }
    while *i < bytes.len() && bytes[*i].is_ascii_digit() {
        *i += 1;
    }
    if *i < bytes.len() && bytes[*i] == b'.' {
        *i += 1;
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(KeyFault::Bad);
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i < bytes.len() && (bytes[*i] == b'e' || bytes[*i] == b'E') {
        *i += 1;
        if *i < bytes.len() && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(KeyFault::Bad);
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i == start {
        return Err(KeyFault::Bad);
    }
    Ok(())
}

fn parse_json_string(bytes: &[u8], i: &mut usize) -> Result<String, KeyFault> {
    if *i >= bytes.len() || bytes[*i] != b'"' {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    let mut out = String::new();
    loop {
        if *i >= bytes.len() {
            return Err(KeyFault::Bad);
        }
        let byte = bytes[*i];
        if byte == b'"' {
            *i += 1;
            return Ok(out);
        }
        if byte == b'\\' {
            *i += 1;
            push_escape(bytes, i, &mut out)?;
            continue;
        }
        if byte <= 0x1f {
            return Err(KeyFault::Bad);
        }
        let start = *i;
        *i += 1;
        while *i < bytes.len() {
            let next = bytes[*i];
            if next == b'"' || next == b'\\' || next <= 0x1f {
                break;
            }
            *i += 1;
        }
        // Stop on ASCII, so the slice does not cut a character. One check
        // per segment keeps the scan linear in the file size.
        let segment = std::str::from_utf8(&bytes[start..*i]).map_err(|_| KeyFault::Bad)?;
        out.push_str(segment);
    }
}

fn push_escape(bytes: &[u8], i: &mut usize, out: &mut String) -> Result<(), KeyFault> {
    if *i >= bytes.len() {
        return Err(KeyFault::Bad);
    }
    let esc = bytes[*i];
    *i += 1;
    match esc {
        b'"' => out.push('"'),
        b'\\' => out.push('\\'),
        b'/' => out.push('/'),
        b'b' => out.push('\u{0008}'),
        b'f' => out.push('\u{000c}'),
        b'n' => out.push('\n'),
        b'r' => out.push('\r'),
        b't' => out.push('\t'),
        b'u' => {
            let unit = hex4(bytes, i)?;
            let code = if (0xD800..=0xDBFF).contains(&unit) {
                if *i + 2 > bytes.len() || bytes[*i] != b'\\' || bytes[*i + 1] != b'u' {
                    return Err(KeyFault::Bad);
                }
                *i += 2;
                let low = hex4(bytes, i)?;
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(KeyFault::Bad);
                }
                0x1_0000 + (((u32::from(unit) - 0xD800) << 10) | (u32::from(low) - 0xDC00))
            } else if (0xDC00..=0xDFFF).contains(&unit) {
                return Err(KeyFault::Bad);
            } else {
                u32::from(unit)
            };
            match char::from_u32(code) {
                Some(ch) => out.push(ch),
                None => return Err(KeyFault::Bad),
            }
        }
        _ => return Err(KeyFault::Bad),
    }
    Ok(())
}

fn hex4(bytes: &[u8], i: &mut usize) -> Result<u16, KeyFault> {
    if *i + 4 > bytes.len() {
        return Err(KeyFault::Bad);
    }
    let hex = std::str::from_utf8(&bytes[*i..*i + 4]).map_err(|_| KeyFault::Bad)?;
    let code = u16::from_str_radix(hex, 16).map_err(|_| KeyFault::Bad)?;
    *i += 4;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::{top_level_keys, top_level_keys_within, KeyFault};

    #[test]
    fn string_scan_joins_a_few_hundred_characters() {
        let plain = "a".repeat(120);
        let mut json = String::from("{\"");
        json.push_str(&plain);
        json.push('é');
        json.push_str(&plain);
        json.push_str("\\n");
        json.push_str(&plain);
        json.push_str(
            "\":1,\"Volumes\":[{\"VolumeId\":\"vol-01234567\",\"Size\":1,\"VolumeType\":\"gp2\"},{\"VolumeId\":\"vol-01234568\",\"Size\":1,\"VolumeType\":\"gp2\"},{\"VolumeId\":\"vol-01234569\",\"Size\":1,\"VolumeType\":\"gp2\"}]}",
        );
        assert!(json.len() < 800);
        let keys = top_level_keys(&json).unwrap_or_else(|_| panic!("scan"));
        let mut expect = String::new();
        expect.push_str(&plain);
        expect.push('é');
        expect.push_str(&plain);
        expect.push('\n');
        expect.push_str(&plain);
        assert_eq!(keys, vec![expect, "Volumes".to_string()]);
    }

    #[test]
    fn two_thousand_distinct_keys_finish_quickly() {
        let mut json = String::with_capacity(24_000);
        json.push('{');
        for n in 0..2000 {
            if n > 0 {
                json.push(',');
            }
            json.push_str(&format!("\"k{n:04}\":1"));
        }
        json.push('}');
        assert!(json.len() < 32_000);
        let started = std::time::Instant::now();
        let keys = top_level_keys(&json).unwrap_or_else(|_| panic!("scan"));
        let elapsed = started.elapsed();
        assert_eq!(keys.len(), 2000);
        assert_eq!(keys[0], "k0000");
        assert_eq!(keys[1999], "k1999");
        assert!(elapsed.as_millis() < 1000, "scan took {elapsed:?}");
    }

    #[test]
    fn surrogate_pair_in_a_string_is_accepted() {
        let json = "{\"Volumes\":[{\"Note\":\"\\ud83d\\ude00\"}]}";
        let keys = top_level_keys(json).unwrap_or_else(|_| panic!("scan"));
        assert_eq!(keys, vec!["Volumes".to_string()]);
    }

    #[test]
    fn deeply_nested_arrays_are_refused_quickly_without_a_panic() {
        for depth in [1000usize, 1_000_000] {
            let mut json = String::from("{\"k\":");
            json.push_str(&"[".repeat(depth));
            let started = std::time::Instant::now();
            assert!(top_level_keys(&json).is_err(), "depth {depth}");
            assert!(started.elapsed().as_millis() < 1000);
        }
    }

    #[test]
    fn duplicate_keys_are_caught_when_nested_or_escaped() {
        let nested = "{\"a\":[{\"x\":1,\"x\":2}]}";
        assert!(matches!(top_level_keys(nested), Err(KeyFault::Duplicate)));
        let escaped = "{\"a\":1,\"\\u0061\":2}";
        assert!(matches!(top_level_keys(escaped), Err(KeyFault::Duplicate)));
        let across_levels = "{\"a\":{\"a\":1},\"b\":[{\"a\":2},{\"a\":3}]}";
        let keys = top_level_keys(across_levels).unwrap_or_else(|_| panic!("scan"));
        assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn non_object_documents_have_no_keys_and_empty_input_is_bad() {
        let keys = top_level_keys(" [1,{\"a\":1}] ").unwrap_or_else(|_| panic!("scan"));
        assert!(keys.is_empty());
        let keys = top_level_keys("\"text\"").unwrap_or_else(|_| panic!("scan"));
        assert!(keys.is_empty());
        assert!(matches!(top_level_keys(""), Err(KeyFault::Bad)));
        assert!(matches!(top_level_keys(" \t\r\n"), Err(KeyFault::Bad)));
        let empty = top_level_keys("{ }").unwrap_or_else(|_| panic!("scan"));
        assert!(empty.is_empty());
    }

    #[test]
    fn malformed_numbers_and_literals_are_bad() {
        for json in [
            "{\"a\":-}",
            "{\"a\":1.}",
            "{\"a\":1e}",
            "{\"a\":1e+}",
            "{\"a\":tru}",
            "{\"a\":nul}",
            "{\"a\":1",
            "{\"a\" 1}",
        ] {
            assert!(matches!(top_level_keys(json), Err(KeyFault::Bad)), "{json}");
        }
        let good = "{\"a\":-1.5e+3,\"b\":true,\"c\":null,\"d\":false}";
        let keys = top_level_keys(good).unwrap_or_else(|_| panic!("scan"));
        assert_eq!(keys.len(), 4);
    }

    #[test]
    fn values_and_keys_are_counted_against_the_maximum() {
        // The object, the key `a`, the array, and the numbers 1 and 2: five items.
        let json = r#"{"a":[1,2]}"#;
        assert!(top_level_keys_within(json, 5).is_ok());
        assert!(matches!(
            top_level_keys_within(json, 4),
            Err(KeyFault::TooMany)
        ));
        // A bare value counts once, and a nested key counts as well.
        assert!(top_level_keys_within("[]", 1).is_ok());
        assert!(matches!(
            top_level_keys_within("[0]", 1),
            Err(KeyFault::TooMany)
        ));
        assert!(top_level_keys_within(r#"[{"k":0}]"#, 4).is_ok());
        assert!(matches!(
            top_level_keys_within(r#"[{"k":0}]"#, 3),
            Err(KeyFault::TooMany)
        ));
    }
}
