//! Detect movetext tokens that pgn-reader otherwise silently skips.
use shakmaty::san::SanPlus;

pub(super) fn validate(input: &str) -> Result<(), String> {
    let bytes = input.as_bytes();
    let mut i = 0;
    let mut variation = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'}' {
                    i += 1;
                }
                i = (i + 1).min(bytes.len());
            }
            b';' | b'%' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'[' => {
                i += 1;
                let mut quoted = false;
                while i < bytes.len() {
                    if bytes[i] == b'\\' && quoted {
                        i = (i + 2).min(bytes.len());
                        continue;
                    }
                    if bytes[i] == b'"' {
                        quoted = !quoted;
                    }
                    if bytes[i] == b']' && !quoted {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'$' => {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            b'(' => {
                variation += 1;
                i += 1;
            }
            b')' => {
                variation = variation.saturating_sub(1);
                i += 1;
            }
            b if b.is_ascii_whitespace() => i += 1,
            _ => {
                let start = i;
                while i < bytes.len()
                    && !bytes[i].is_ascii_whitespace()
                    && !b"{}();[$".contains(&bytes[i])
                {
                    i += 1;
                }
                if variation != 0 {
                    continue;
                }
                let mut token = &input[start..i];
                if token.starts_with('$') && token[1..].bytes().all(|b| b.is_ascii_digit()) {
                    continue;
                }
                if matches!(token, "1-0" | "0-1" | "1/2-1/2" | "½-½" | "*") {
                    continue;
                }
                token = token.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
                token = token.trim_end_matches(['!', '?']);
                if !token.is_empty() && SanPlus::from_ascii(token.as_bytes()).is_err() {
                    return Err(format!("Lance inválido: {token}."));
                }
            }
        }
    }
    Ok(())
}
