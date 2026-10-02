//! Reads a suite file into [`Value`]s whose numbers hold the text they are written with.
//!
//! A case's input is a value of the input model, in which a number is its lexeme
//! (spec/input-model.md). serde_json's parser keeps a number's text with `arbitrary_precision`,
//! except that it reads an integer as an `i64` or `u64` first and writes that back: `-0` becomes
//! `0`, and `double` would be given +0 where the case writes -0. A `Value` can hold `-0`; only the
//! parser loses it. So the runner reads the suite with this reader, which gives every number the
//! exact text the file has, as a runner has to give its decoder the exact text of the input.

use serde_json::{Map, Number, Value};

pub fn parse(text: &str) -> Result<Value, String> {
    let mut reader = Reader {
        text: text.as_bytes(),
        at: 0,
    };
    let value = reader.value()?;
    reader.space();
    if reader.at != reader.text.len() {
        return Err(reader.error("trailing text"));
    }
    Ok(value)
}

struct Reader<'a> {
    text: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn error(&self, what: &str) -> String {
        format!("{what} at byte {}", self.at)
    }

    fn space(&mut self) {
        while self
            .text
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
        {
            self.at += 1;
        }
    }

    fn eat(&mut self, b: u8) -> Result<(), String> {
        self.space();
        if self.text.get(self.at) == Some(&b) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected {}", b as char)))
        }
    }

    fn word(&mut self, word: &str, value: Value) -> Result<Value, String> {
        if self.text[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.error("not JSON"))
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        self.space();
        match self.text.get(self.at) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.word("true", Value::Bool(true)),
            Some(b'f') => self.word("false", Value::Bool(false)),
            Some(b'n') => self.word("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.error("not JSON")),
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.at += 1;
        let mut members = Map::new();
        self.space();
        if self.text.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.space();
            let name = self.string()?;
            self.eat(b':')?;
            let value = self.value()?;
            if members.insert(name, value).is_some() {
                return Err(self.error("a member name twice"));
            }
            self.space();
            match self.text.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(self.error("expected , or }")),
            }
        }
    }

    fn array(&mut self) -> Result<Value, String> {
        self.at += 1;
        let mut items = Vec::new();
        self.space();
        if self.text.get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.space();
            match self.text.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected , or ]")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .text
            .get(self.at..self.at + 4)
            .ok_or_else(|| self.error("a short \\u escape"))?;
        let digits = std::str::from_utf8(digits).map_err(|e| e.to_string())?;
        let unit = u32::from_str_radix(digits, 16).map_err(|_| self.error("a bad \\u escape"))?;
        self.at += 4;
        Ok(unit)
    }

    fn string(&mut self) -> Result<String, String> {
        if self.text.get(self.at) != Some(&b'"') {
            return Err(self.error("expected a string"));
        }
        self.at += 1;
        let mut out = String::new();
        loop {
            let start = self.at;
            while self
                .text
                .get(self.at)
                .is_some_and(|b| *b != b'"' && *b != b'\\')
            {
                self.at += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.text[start..self.at]).map_err(|e| e.to_string())?,
            );
            match self.text.get(self.at) {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    let escape = *self
                        .text
                        .get(self.at)
                        .ok_or_else(|| self.error("an unfinished escape"))?;
                    self.at += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut unit = self.hex4()?;
                            if (0xD800..0xDC00).contains(&unit)
                                && self.text[self.at..].starts_with(b"\\u")
                            {
                                self.at += 2;
                                let low = self.hex4()?;
                                unit = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                            }
                            out.push(
                                char::from_u32(unit)
                                    .ok_or_else(|| self.error("an unpaired surrogate"))?,
                            );
                        }
                        _ => return Err(self.error("a bad escape")),
                    }
                }
                _ => return Err(self.error("an unterminated string")),
            }
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.at;
        while self
            .text
            .get(self.at)
            .is_some_and(|b| matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
        {
            self.at += 1;
        }
        let lexeme = std::str::from_utf8(&self.text[start..self.at]).map_err(|e| e.to_string())?;
        // serde_json checks the grammar; its reading of the value is not kept.
        serde_json::from_str::<Value>(lexeme).map_err(|_| self.error("a bad number"))?;
        Ok(Value::Number(Number::from_string_unchecked(
            lexeme.to_owned(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_keep_their_text() {
        let value = parse(r#"{"a": -0, "b": [1.50, 1E+3, 12345678901234567890123]}"#).unwrap();
        assert_eq!(value["a"].to_string(), "-0");
        assert_eq!(
            value["b"].to_string(),
            "[1.50,1E+3,12345678901234567890123]"
        );
    }

    #[test]
    fn strings_read_their_escapes() {
        let value = parse(r#""aé😀\n""#).unwrap();
        assert_eq!(value, Value::String("aé😀\n".into()));
    }
}
