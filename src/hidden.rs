//! One list of the characters that are shown as `\u{..}` escapes: line breaks,
//! controls, invisible characters, spacing characters other than the plain
//! space, and characters that change the text direction.

/// True for a character that must not reach a terminal as itself.
/// This is a list, not a Unicode category: a character that is not in it and
/// is not a control or white space is shown as itself.
pub fn is_hidden(c: char) -> bool {
    c.is_control()
        || (c.is_whitespace() && c != ' ')
        || matches!(c,
            '\u{600}'..='\u{605}' | '\u{6DD}' | '\u{70F}' | '\u{8E2}'
            | '\u{1D173}'..='\u{1D17A}' | '\u{FFFC}',
        )
        || matches!(c,
            '\u{AD}' | '\u{34F}' | '\u{61C}' | '\u{115F}' | '\u{1160}' | '\u{17B4}' | '\u{17B5}'
            | '\u{180B}'..='\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{2028}' | '\u{2029}' | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{2800}' | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}' | '\u{FFA0}' | '\u{FFF9}'..='\u{FFFB}'
            | '\u{E0000}'..='\u{E007F}' | '\u{E0100}'..='\u{E01EF}')
}

/// The text with every hidden character written as `\u{..}`.
pub fn escape_hidden(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if is_hidden(c) {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{escape_hidden, is_hidden};

    #[test]
    fn the_list_has_controls_direction_and_invisible_characters() {
        for c in [
            '\n',
            '\r',
            '\u{1b}',
            '\u{7f}',
            '\u{85}',
            '\u{AD}',
            '\u{61C}',
            '\u{200B}',
            '\u{200E}',
            '\u{2028}',
            '\u{202E}',
            '\u{2066}',
            '\u{2800}',
            '\u{3164}',
            '\u{FE0F}',
            '\u{FEFF}',
            '\u{E0001}',
            '\u{E0100}',
            '\u{E01EF}',
        ] {
            assert!(is_hidden(c), "{:04X}", c as u32);
        }
        for c in ['a', ' ', 'é', '\u{E01F0}', '\u{2027}', '\u{2070}'] {
            assert!(!is_hidden(c), "{:04X}", c as u32);
        }
    }

    #[test]
    fn spacing_and_format_characters_are_hidden() {
        for c in [
            '\u{A0}',
            '\u{1680}',
            '\u{2000}',
            '\u{2003}',
            '\u{200A}',
            '\u{202F}',
            '\u{205F}',
            '\u{3000}',
            '\u{600}',
            '\u{605}',
            '\u{6DD}',
            '\u{70F}',
            '\u{8E2}',
            '\u{1D173}',
            '\u{1D17A}',
            '\u{FFFC}',
        ] {
            assert!(is_hidden(c), "{:04X}", c as u32);
        }
        for c in ['\u{606}', '\u{1D172}', '\u{1D17B}', '\u{FFFD}', ' ', 'x'] {
            assert!(!is_hidden(c), "{:04X}", c as u32);
        }
    }

    #[test]
    fn escape_hidden_keeps_plain_text() {
        assert_eq!(escape_hidden("a\u{1b}[0m\nb"), "a\\u{1b}[0m\\u{a}b");
        assert_eq!(escape_hidden("plain é"), "plain é");
    }
}
