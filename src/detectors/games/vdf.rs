//! Minimal parser for Valve's text KeyValues format (VDF / ACF files).
//!
//! Handles quoted and unquoted tokens, `\"` and `\\` escapes, nested
//! blocks, `//` comments and `[$CONDITION]` suffixes. Anything malformed
//! yields `None` rather than a partial result.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
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
            Value::Str(_) => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Obj(_) => None,
        }
    }

    pub fn children(&self) -> &[(String, Value)] {
        match self {
            Value::Obj(items) => items,
            Value::Str(_) => &[],
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
    fn malformed_input_is_none() {
        assert!(parse("\"a\" { \"b\" \"c\"").is_none());
        assert!(parse("\"unterminated").is_none());
        assert!(parse("\"a\" }").is_none());
    }
}
