pub fn decode(input: &str) -> Result<String, String> {
    if input.len() < 2 || !input.starts_with('"') || !input.ends_with('"') {
        return Err("Expected quoted string".into());
    }

    let inner = &input[1..input.len() - 1];
    let mut result = String::new();
    let mut chars = inner.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            if ch == '"' {
                return Err("Unescaped quote".into());
            }

            if ch.is_control() {
                return Err("Unescaped control character".into());
            }

            result.push(ch);
            continue;
        }

        let escaped = chars.next().ok_or("Incomplete escape sequence")?;

        match escaped {
            '"' => result.push('"'),
            '\\' => result.push('\\'),
            'n' => result.push('\n'),
            'r' => result.push('\r'),
            't' => result.push('\t'),
            '0' => result.push('\0'),

            'u' => {
                if chars.next() != Some('{') {
                    return Err("Expected '{' after \\u".into());
                }

                let mut hex = String::new();
                let mut closed = false;

                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }

                    hex.push(c);
                    if hex.len() > 6 {
                        return Err("Invalid Unicode escape".into());
                    }
                }

                if !closed
                    || hex.is_empty()
                    || hex.len() > 6
                    || !hex.chars().all(|c| c.is_ascii_hexdigit())
                {
                    return Err("Invalid Unicode escape".into());
                }

                let code =
                    u32::from_str_radix(&hex, 16).map_err(|_| "Invalid Unicode code point")?;

                let character = char::from_u32(code).ok_or("Invalid Unicode scalar value")?;

                result.push(character);
            }

            _ => {
                return Err(format!("Unknown escape: \\{escaped}"));
            }
        }
    }

    Ok(result)
}

pub fn encode(input: &str) -> String {
    let mut result = String::from("\"");

    for ch in input.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\0' => result.push_str("\\0"),

            c if c.is_control() => {
                result.push_str(&format!("\\u{{{:X}}}", c as u32));
            }

            c => result.push(c),
        }
    }

    result.push('"');
    result
}
