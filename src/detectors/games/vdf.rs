//! Minimal parser for Valve's text KeyValues format (VDF / ACF files).
//!
//! Handles quoted and unquoted tokens, `\"` and `\\` escapes, nested
//! blocks, `//` comments and `[$CONDITION]` suffixes. Anything malformed
//! yields `None` rather than a partial result.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    /// Only produced by the binary format.
    Int(i64),
    Obj(Vec<(String, Value)>),
}

impl Value {
    /// Case-insensitive lookup of a direct child, as Steam itself does.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(items) => items
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn children(&self) -> &[(String, Value)] {
        match self {
            Value::Obj(items) => items,
            _ => &[],
        }
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Str(String),
    Open,
    Close,
}

fn tokenize(input: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '{' => {
                chars.next();
                tokens.push(Token::Open);
            }
            '}' => {
                chars.next();
                tokens.push(Token::Close);
            }
            '/' => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                } else {
                    return None;
                }
            }
            '[' => {
                // Conditional like [$WIN32]; ignore.
                for c in chars.by_ref() {
                    if c == ']' {
                        break;
                    }
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => match chars.next()? {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            other => s.push(other),
                        },
                        other => s.push(other),
                    }
                }
                tokens.push(Token::Str(s));
            }
            _ => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || matches!(c, '{' | '}' | '"') {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                tokens.push(Token::Str(s));
            }
        }
    }
    Some(tokens)
}

fn parse_items(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    nested: bool,
) -> Option<Vec<(String, Value)>> {
    let mut items = Vec::new();
    loop {
        match tokens.next() {
            None if !nested => return Some(items),
            None => return None,
            Some(Token::Close) if nested => return Some(items),
            Some(Token::Str(key)) => match tokens.next()? {
                Token::Str(v) => items.push((key, Value::Str(v))),
                Token::Open => items.push((key, Value::Obj(parse_items(tokens, true)?))),
                Token::Close => return None,
            },
            Some(_) => return None,
        }
    }
}

/// Parses a whole VDF document into a top-level object.
pub fn parse(input: &str) -> Option<Value> {
    let mut tokens = tokenize(input)?.into_iter().peekable();
    Some(Value::Obj(parse_items(&mut tokens, false)?))
}

/// Parses Steam's binary KeyValues format (used by `shortcuts.vdf`).
///
/// Each entry is a type byte, a NUL-terminated key, then the value:
/// `0x00` nested object (until `0x08`), `0x01` NUL-terminated string,
/// `0x02` 32-bit int, `0x03` 32-bit float, `0x07` 64-bit int.
pub fn parse_binary(data: &[u8]) -> Option<Value> {
    let mut pos = 0;
    let items = parse_binary_items(data, &mut pos, 0)?;
    Some(Value::Obj(items))
}

fn read_cstr(data: &[u8], pos: &mut usize) -> Option<String> {
    let len = data.get(*pos..)?.iter().position(|&b| b == 0)?;
    let s = String::from_utf8_lossy(&data[*pos..*pos + len]).into_owned();
    *pos += len + 1;
    Some(s)
}

fn read_bytes<const N: usize>(data: &[u8], pos: &mut usize) -> Option<[u8; N]> {
    let bytes = data.get(*pos..*pos + N)?.try_into().ok()?;
    *pos += N;
    Some(bytes)
}

fn parse_binary_items(data: &[u8], pos: &mut usize, depth: usize) -> Option<Vec<(String, Value)>> {
    if depth > 32 {
        return None;
    }
    let mut items = Vec::new();
    loop {
        let Some(&kind) = data.get(*pos) else {
            // End of input is only valid at the top level.
            return (depth == 0).then_some(items);
        };
        *pos += 1;
        if kind == 0x08 {
            return Some(items);
        }
        let key = read_cstr(data, pos)?;
        let value = match kind {
            0x00 => Value::Obj(parse_binary_items(data, pos, depth + 1)?),
            0x01 => Value::Str(read_cstr(data, pos)?),
            0x02 => Value::Int(i32::from_le_bytes(read_bytes(data, pos)?) as i64),
            0x03 => Value::Str(f32::from_le_bytes(read_bytes(data, pos)?).to_string()),
            0x07 => Value::Int(u64::from_le_bytes(read_bytes(data, pos)?) as i64),
            _ => return None,
        };
        items.push((key, value));
    }
}

/// Convenience: flat string fields of `doc[root]`, lower-cased keys.
pub fn string_fields(doc: &Value, root: &str) -> HashMap<String, String> {
    doc.get(root)
        .map(|r| {
            r.children()
                .iter()
                .filter_map(|(k, v)| Some((k.to_ascii_lowercase(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_acf() {
        let acf = r#"
"AppState"
{
	"appid"		"1245620"
	"Universe"		"1"
	"name"		"ELDEN RING"
	"InstalledDepots"
	{
		"1245621" { "manifest" "123" }
	}
}"#;
        let doc = parse(acf).unwrap();
        let f = string_fields(&doc, "AppState");
        assert_eq!(f["appid"], "1245620");
        assert_eq!(f["name"], "ELDEN RING");
    }

    #[test]
    fn handles_escapes_and_comments() {
        let vdf =
            "// header\n\"libraryfolders\" { \"0\" { \"path\" \"/mnt/My \\\"Games\\\"\\\\x\" } }";
        let doc = parse(vdf).unwrap();
        let path = doc
            .get("libraryfolders")
            .and_then(|l| l.get("0"))
            .and_then(|f| f.get("path"))
            .and_then(Value::as_str);
        assert_eq!(path, Some("/mnt/My \"Games\"\\x"));
    }

    #[test]
    fn parses_binary_shortcuts() {
        let mut d = Vec::new();
        d.extend(b"\x00shortcuts\x00\x000\x00");
        d.extend(b"\x02appid\x00");
        d.extend(&(-1_234_567_890i32).to_le_bytes());
        d.extend(b"\x01AppName\x00Dolphin\x00\x01Exe\x00\"/usr/bin/dolphin\"\x00");
        d.extend(b"\x00tags\x00\x08");
        d.extend(b"\x08\x08\x08");
        let doc = parse_binary(&d).unwrap();
        let entry = doc.get("shortcuts").unwrap().get("0").unwrap();
        assert_eq!(
            entry.get("appname").and_then(Value::as_str),
            Some("Dolphin")
        );
        assert_eq!(
            entry.get("appid").and_then(Value::as_int),
            Some(-1_234_567_890)
        );
    }

    #[test]
    fn truncated_binary_is_none() {
        assert!(parse_binary(b"\x00shortcuts\x00\x02appid\x00\x01\x02").is_none());
        assert!(parse_binary(b"\x00shortcuts\x00\x01x\x00no-terminator").is_none());
        assert!(parse_binary(b"\x09odd\x00").is_none());
    }

    #[test]
    fn malformed_input_is_none() {
        assert!(parse("\"a\" { \"b\" \"c\"").is_none());
        assert!(parse("\"unterminated").is_none());
        assert!(parse("\"a\" }").is_none());
    }
}
