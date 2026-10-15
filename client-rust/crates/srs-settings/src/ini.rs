//! Reader/writer for the SharpConfig 4.1 text format (the library the C#
//! client uses). Behaviour copied from `ConfigurationReader.cs`,
//! `ConfigurationWriter.cs`, `Setting.cs` and `StockStringConverters.cs`.
//!
//! Format summary:
//! - `[Section]` headers; settings before any header belong to the unnamed
//!   default section (written first, without header).
//! - `Name=Value` (no spaces around `=` by default). Whitespace around both is
//!   trimmed when reading.
//! - `#` and `;` start a comment unless inside `"` quotes or escaped with `\`.
//!   Whole-line comments before an element are its "pre-comment"; a trailing
//!   one is its inline comment. Both are preserved on write.
//! - On write, a value containing a space or a comment character is wrapped
//!   in double quotes unless it is already quoted, an array `{...}`, or a
//!   multiline `[[...]]` block. `StringValue` strips surrounding quotes.
//! - Arrays: `{a,b,c}`; booleans: `True`/`False`; numbers: invariant culture.
//! - One blank line between sections. Lines end with the platform newline in
//!   C# (`\r\n` on Windows); we always write `\r\n` because the files live on
//!   Windows and the C# reader accepts both.

use std::fmt::Write as _;

/// Newline written by this crate. The C# writer uses `Environment.NewLine`.
pub const NEWLINE: &str = "\r\n";

const COMMENT_CHARS: [char; 2] = ['#', ';'];
const PREFERRED_COMMENT_CHAR: char = '#';
const ARRAY_SEPARATOR: char = ',';

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {message}")]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Setting {
    pub name: String,
    /// Exactly what sits after `=`, trimmed, quotes included if any.
    pub raw: String,
    pub pre_comment: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Section {
    /// `None` for the unnamed default section.
    pub name: Option<String>,
    pub settings: Vec<Setting>,
    pub pre_comment: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    pub sections: Vec<Section>,
}

// ---------------------------------------------------------------------------
// Value conversions (StockStringConverters + Setting.GetValueForOutput)
// ---------------------------------------------------------------------------

impl Setting {
    pub fn new(name: impl Into<String>, raw: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            raw: raw.into(),
            pre_comment: None,
            comment: None,
        }
    }

    /// `Setting.StringValue` getter: raw value with surrounding `"` trimmed
    /// (multiline `[[...]]` values are returned verbatim).
    pub fn string_value(&self) -> &str {
        if self.raw.starts_with("[[") && self.raw.ends_with("]]") {
            return &self.raw;
        }
        self.raw.trim_matches('"')
    }

    /// `BoolStringConverter`: `true/on/yes/y/1` and `false/off/no/n/0/""`,
    /// case-insensitive. Anything else is `None`.
    pub fn bool_value(&self) -> Option<bool> {
        match self.string_value().to_ascii_lowercase().as_str() {
            "" | "false" | "off" | "no" | "n" | "0" => Some(false),
            "true" | "on" | "yes" | "y" | "1" => Some(true),
            _ => None,
        }
    }

    pub fn i32_value(&self) -> Option<i32> {
        let s = self.string_value().trim();
        if s.is_empty() {
            return Some(0);
        }
        s.parse().ok()
    }

    pub fn f64_value(&self) -> Option<f64> {
        let s = self.string_value().trim();
        if s.is_empty() {
            return Some(0.0);
        }
        s.parse().ok()
    }

    pub fn f32_value(&self) -> Option<f32> {
        self.f64_value().map(|v| v as f32)
    }

    /// `Setting.IsEmpty`.
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// `Setting.StringValueArray`: `None` when the value is not `{...}`.
    /// Elements are trimmed of spaces and of one pair of surrounding quotes.
    pub fn string_array(&self) -> Option<Vec<String>> {
        parse_array(&self.raw)
    }

    pub fn set_string(&mut self, value: &str) {
        // StringValue setter trims quotes, then output quoting applies at write time.
        self.raw = value.trim_matches('"').to_string();
    }

    /// `RawValue` setter: no processing at all.
    pub fn set_raw(&mut self, value: &str) {
        self.raw = value.to_string();
    }

    pub fn set_bool(&mut self, value: bool) {
        self.raw = if value { "True" } else { "False" }.to_string();
    }

    pub fn set_i32(&mut self, value: i32) {
        self.raw = value.to_string();
    }

    /// `double.ToString(InvariantCulture)`: shortest round-trip form, which
    /// is what Rust's `Display` produces for finite values (`1`, `0.514`).
    pub fn set_f64(&mut self, value: f64) {
        self.raw = format_dotnet_double(value);
    }

    pub fn set_f32(&mut self, value: f32) {
        self.raw = format_dotnet_double(f64::from(value));
    }

    pub fn set_string_array(&mut self, values: &[String]) {
        let items: Vec<String> = values.iter().map(|v| quote_for_output(v)).collect();
        self.raw = format!("{{{}}}", items.join(&ARRAY_SEPARATOR.to_string()));
    }

    fn write_to(&self, out: &mut String) {
        if let Some(pre) = &self.pre_comment {
            write_pre_comment(out, pre);
        }
        out.push_str(&self.name);
        out.push('=');
        out.push_str(&quote_for_output(&self.raw));
        if let Some(c) = &self.comment {
            out.push(' ');
            out.push(PREFERRED_COMMENT_CHAR);
            out.push(' ');
            out.push_str(c);
        }
        out.push_str(NEWLINE);
    }
}

/// .NET Core `double.ToString()` with the invariant culture. Rust's shortest
/// round-trip representation matches for the usual magnitudes; very small or
/// very large values would use `E` notation in .NET (not needed here).
pub fn format_dotnet_double(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    format!("{v}")
}

/// `Setting.GetValueForOutput`.
fn quote_for_output(raw: &str) -> String {
    if (raw.starts_with('{') && raw.ends_with('}'))
        || (raw.starts_with('"') && raw.ends_with('"'))
        || (raw.starts_with("[[") && raw.ends_with("]]"))
    {
        return raw.to_string();
    }
    if raw.contains(' ') || raw.chars().any(|c| COMMENT_CHARS.contains(&c)) {
        return format!("\"{raw}\"");
    }
    raw.to_string()
}

fn write_pre_comment(out: &mut String, pre: &str) {
    for line in pre.split('\n') {
        let line = line.trim_end_matches('\r');
        out.push(PREFERRED_COMMENT_CHAR);
        out.push(' ');
        out.push_str(line);
        out.push_str(NEWLINE);
    }
}

/// Port of `SettingArrayEnumerator` (the parts needed for string arrays).
fn parse_array(raw: &str) -> Option<Vec<String>> {
    let chars: Vec<char> = raw.chars().collect();
    // Leading: spaces then '{'.
    let mut start = None;
    for (i, &c) in chars.iter().enumerate() {
        if c != ' ' && c != '{' {
            break;
        }
        if c == '{' {
            start = Some(i + 1);
            break;
        }
    }
    let start = start?;
    let mut last_brace = None;
    for i in (0..chars.len()).rev() {
        let c = chars[i];
        if c != ' ' && c != '}' {
            break;
        }
        if c == '}' {
            last_brace = Some(i);
            break;
        }
    }
    let last_brace = last_brace?;
    if start >= last_brace || chars[start..last_brace].iter().all(|c| *c == ' ') {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    let mut depth = 1;
    let mut in_quotes = false;
    let mut elem_start = start;
    let mut i = start;
    while i <= last_brace {
        let c = chars[i];
        if c == '{' && !in_quotes {
            depth += 1;
        } else if c == '}' && !in_quotes {
            depth -= 1;
            if i == last_brace {
                let e = element(&chars[elem_start..i]);
                if e.is_empty() {
                    return None;
                }
                out.push(e);
                break;
            }
        } else if c == '"' {
            in_quotes = !in_quotes;
        } else if c == ARRAY_SEPARATOR && depth == 1 && !in_quotes {
            let e = element(&chars[elem_start..i]);
            if e.is_empty() {
                return None;
            }
            out.push(e);
            elem_start = i + 1;
        }
        i += 1;
    }
    if in_quotes {
        return None;
    }
    Some(out)
}

fn element(chars: &[char]) -> String {
    let mut s: String = chars.iter().collect();
    s = s.trim_matches(' ').to_string();
    if s.ends_with('"') {
        s.pop();
    }
    if s.starts_with('"') {
        s.remove(0);
    }
    s
}

// ---------------------------------------------------------------------------
// Section / Document
// ---------------------------------------------------------------------------

impl Section {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Default::default()
        }
    }

    pub fn get(&self, name: &str) -> Option<&Setting> {
        self.settings.iter().find(|s| s.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Setting> {
        self.settings.iter_mut().find(|s| s.name == name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Get the setting, creating it with `raw` when absent. Returns whether
    /// it was created (callers save the file in that case, like the C# code).
    pub fn get_or_insert(&mut self, name: &str, raw: &str) -> (&mut Setting, bool) {
        if let Some(i) = self.settings.iter().position(|s| s.name == name) {
            (&mut self.settings[i], false)
        } else {
            self.settings.push(Setting::new(name, raw));
            let i = self.settings.len() - 1;
            (&mut self.settings[i], true)
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.settings.len();
        self.settings.retain(|s| s.name != name);
        before != self.settings.len()
    }

    fn write_to(&self, out: &mut String, first: bool) {
        if !first {
            out.push_str(NEWLINE);
        }
        if !first && self.pre_comment.is_some() {
            out.push_str(NEWLINE);
        }
        if let Some(name) = &self.name {
            if let Some(pre) = &self.pre_comment {
                write_pre_comment(out, pre);
            }
            out.push('[');
            out.push_str(name);
            out.push(']');
            if let Some(c) = &self.comment {
                out.push(' ');
                out.push(PREFERRED_COMMENT_CHAR);
                out.push(' ');
                out.push_str(c);
            }
            out.push_str(NEWLINE);
        }
        for s in &self.settings {
            s.write_to(out);
        }
    }
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name.as_deref() == Some(name))
    }

    pub fn section_mut(&mut self, name: &str) -> Option<&mut Section> {
        self.sections.iter_mut().find(|s| s.name.as_deref() == Some(name))
    }

    pub fn contains_section(&self, name: &str) -> bool {
        self.section(name).is_some()
    }

    /// Named section, appended if missing.
    pub fn section_or_insert(&mut self, name: &str) -> &mut Section {
        if let Some(i) = self.sections.iter().position(|s| s.name.as_deref() == Some(name)) {
            &mut self.sections[i]
        } else {
            self.sections.push(Section::new(name));
            let i = self.sections.len() - 1;
            &mut self.sections[i]
        }
    }

    pub fn remove_section(&mut self, name: &str) -> bool {
        let before = self.sections.len();
        self.sections.retain(|s| s.name.as_deref() != Some(name));
        before != self.sections.len()
    }

    /// Parse text. A UTF-8 BOM, if present, must be stripped by the caller
    /// (see [`Document::parse_bytes`]).
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut doc = Document::new();
        let mut current = Section::default();
        let mut pre_comment: Vec<String> = Vec::new();
        let mut lines = text.lines().enumerate().peekable();

        while let Some((idx, raw_line)) = lines.next() {
            let line_no = idx + 1;
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            let (comment, comment_idx) = parse_comment(line);
            if comment_idx == Some(0) {
                pre_comment.push(comment.unwrap_or_default());
                continue;
            }
            let without_comment = match comment_idx {
                Some(i) => line[..i].trim(),
                None => line,
            };

            if without_comment.starts_with('[') {
                if current.name.is_none() && !current.settings.is_empty() {
                    doc.sections.push(std::mem::take(&mut current));
                }
                let mut section = parse_section(without_comment, line_no)?;
                section.comment = comment;
                if !pre_comment.is_empty() {
                    section.pre_comment = Some(pre_comment.join("\n"));
                    pre_comment.clear();
                }
                // Push previous named section.
                if current.name.is_some() {
                    doc.sections.push(std::mem::take(&mut current));
                }
                current = section;
            } else {
                let mut setting = parse_setting(without_comment, line_no, &mut lines)?;
                setting.comment = comment;
                if !pre_comment.is_empty() {
                    setting.pre_comment = Some(pre_comment.join("\n"));
                    pre_comment.clear();
                }
                current.settings.push(setting);
            }
        }
        if current.name.is_some() || !current.settings.is_empty() {
            doc.sections.push(current);
        }
        Ok(doc)
    }

    /// Parse raw file bytes: strips a UTF-8 BOM, decodes lossily.
    pub fn parse_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        let text = String::from_utf8_lossy(bytes);
        Self::parse(&text)
    }

    /// Serialise exactly like `ConfigurationWriter.WriteToTextWriter`.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let mut first = true;
        // Default (unnamed) section first, only if it has settings.
        for s in self.sections.iter().filter(|s| s.name.is_none()) {
            if !s.settings.is_empty() {
                s.write_to(&mut out, first);
                first = false;
            }
        }
        for s in self.sections.iter().filter(|s| s.name.is_some()) {
            s.write_to(&mut out, first);
            first = false;
        }
        out
    }

    /// Bytes to write, with or without a UTF-8 BOM. `global.cfg` is written
    /// without (`new UTF8Encoding(false)`), profiles with (`Encoding.UTF8`).
    pub fn to_bytes(&self, with_bom: bool) -> Vec<u8> {
        let text = self.to_text();
        let mut v = Vec::with_capacity(text.len() + 3);
        if with_bom {
            v.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        v.extend_from_slice(text.as_bytes());
        v
    }
}

/// Returns (comment text, index of the comment char) like `ParseComment`.
fn parse_comment(line: &str) -> (Option<String>, Option<usize>) {
    let bytes: Vec<(usize, char)> = line.char_indices().collect();
    let mut quote_count = 0usize;
    for (k, &(idx, c)) in bytes.iter().enumerate() {
        let escaped = k > 0 && bytes[k - 1].1 == '\\';
        if COMMENT_CHARS.contains(&c) && quote_count % 2 == 0 && !escaped {
            let after = &line[idx + c.len_utf8()..];
            return (Some(after.trim_start().to_string()), Some(idx));
        }
        if c == '"' && !escaped {
            quote_count += 1;
        }
    }
    (None, None)
}

fn parse_section(line: &str, line_no: usize) -> Result<Section, ParseError> {
    let close = line.rfind(']').ok_or_else(|| ParseError {
        line: line_no,
        message: "Closing bracket missing.".into(),
    })?;
    let name = line[1..close].trim().to_string();
    let rest = line[close + 1..].trim();
    if !rest.is_empty() {
        return Err(ParseError {
            line: line_no,
            message: format!("Unexpected token: '{rest}'"),
        });
    }
    Ok(Section::new(name))
}

fn parse_setting<'a, I>(line: &str, line_no: usize, lines: &mut I) -> Result<Setting, ParseError>
where
    I: Iterator<Item = (usize, &'a str)>,
{
    let (name, rest) = if let Some(stripped) = line.strip_prefix('"') {
        // Quoted name: find closing unescaped quote.
        let mut close = None;
        let mut prev = ' ';
        for (i, c) in stripped.char_indices() {
            if c == '"' && prev != '\\' {
                close = Some(i);
                break;
            }
            prev = c;
        }
        let close = close.ok_or_else(|| ParseError {
            line: line_no,
            message: "Closing quote mark expected.".into(),
        })?;
        let name = stripped[..close].to_string();
        let after = &stripped[close + 1..];
        let eq = after.find('=').ok_or_else(|| ParseError {
            line: line_no,
            message: "Setting assignment expected.".into(),
        })?;
        (name, &after[eq + 1..])
    } else {
        let eq = line.find('=').ok_or_else(|| ParseError {
            line: line_no,
            message: "Setting assignment expected.".into(),
        })?;
        (line[..eq].trim().to_string(), &line[eq + 1..])
    };
    if name.is_empty() {
        return Err(ParseError {
            line: line_no,
            message: "Setting name expected.".into(),
        });
    }
    let mut value = rest.trim().to_string();
    if value == "[[" {
        let mut buf = String::from("[[\n");
        let mut closed = false;
        for (_, l) in lines.by_ref() {
            if l == "]]" {
                closed = true;
                break;
            }
            let _ = writeln!(buf, "{l}");
        }
        if !closed {
            return Err(ParseError {
                line: line_no,
                message: "Closing multiline tag ']]' expected.".into(),
            });
        }
        buf.push_str("]]");
        value = buf;
    }
    Ok(Setting::new(name, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "[Position Settings]\r\nRadioX=300\r\nRadioY=300\r\n\r\n[Client Settings]\r\nAutoConnect=True\r\nLastServer=127.0.0.1\r\nSettingsProfiles={default.cfg,F18_1.cfg}\r\nLastSeenName=\"Call Sign\"\r\nSpeakerBoost=0.514\r\n\r\n[Network Settings]\r\nDCSIncomingUDP=9084\r\n";

    #[test]
    fn roundtrip_is_byte_identical() {
        let doc = Document::parse(SAMPLE).unwrap();
        assert_eq!(doc.to_text(), SAMPLE);
        let with_bom = doc.to_bytes(true);
        assert_eq!(&with_bom[..3], &[0xEF, 0xBB, 0xBF]);
        assert_eq!(Document::parse_bytes(&with_bom).unwrap(), doc);
    }

    #[test]
    fn typed_values() {
        let doc = Document::parse(SAMPLE).unwrap();
        let cs = doc.section("Client Settings").unwrap();
        assert_eq!(cs.get("AutoConnect").unwrap().bool_value(), Some(true));
        assert_eq!(cs.get("LastSeenName").unwrap().string_value(), "Call Sign");
        assert_eq!(cs.get("LastSeenName").unwrap().raw, "\"Call Sign\"");
        assert_eq!(cs.get("SpeakerBoost").unwrap().f64_value(), Some(0.514));
        assert_eq!(
            cs.get("SettingsProfiles").unwrap().string_array().unwrap(),
            vec!["default.cfg", "F18_1.cfg"]
        );
        assert_eq!(
            doc.section("Network Settings")
                .unwrap()
                .get("DCSIncomingUDP")
                .unwrap()
                .i32_value(),
            Some(9084)
        );
        assert_eq!(Setting::new("x", "").i32_value(), Some(0));
        assert_eq!(Setting::new("x", "abc").i32_value(), None);
        assert_eq!(
            Setting::new("x", "{}").string_array().unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(Setting::new("x", "{a,,b}").string_array(), None);
        assert_eq!(Setting::new("x", "plain").string_array(), None);
    }

    #[test]
    fn setters_format_like_dotnet() {
        let mut s = Setting::new("x", "");
        s.set_bool(true);
        assert_eq!(s.raw, "True");
        s.set_f64(1.0);
        assert_eq!(s.raw, "1");
        s.set_f64(0.514);
        assert_eq!(s.raw, "0.514");
        s.set_f32(1.2);
        assert_eq!(s.raw, "1.2000000476837158");
        s.set_i32(-30);
        assert_eq!(s.raw, "-30");
        s.set_string_array(&["default.cfg".into(), "My Jet.cfg".into()]);
        assert_eq!(s.raw, "{default.cfg,\"My Jet.cfg\"}");
        s.set_string("has space");
        let mut out = String::new();
        s.write_to(&mut out);
        assert_eq!(out, "x=\"has space\"\r\n");
        s.set_string("Tom#Cat");
        let mut out = String::new();
        s.write_to(&mut out);
        assert_eq!(out, "x=\"Tom#Cat\"\r\n");
    }

    #[test]
    fn comments_and_default_section() {
        let text = "# top\r\nfoo=1 ; inline\r\n[S] # sec\r\n; pre\r\nbar = 2\r\n";
        let doc = Document::parse(text).unwrap();
        assert_eq!(doc.sections.len(), 2);
        assert_eq!(doc.sections[0].name, None);
        assert_eq!(doc.sections[0].settings[0].pre_comment.as_deref(), Some("top"));
        assert_eq!(doc.sections[0].settings[0].comment.as_deref(), Some("inline"));
        assert_eq!(doc.sections[0].settings[0].raw, "1");
        assert_eq!(doc.sections[1].comment.as_deref(), Some("sec"));
        assert_eq!(doc.sections[1].settings[0].name, "bar");
        assert_eq!(doc.sections[1].settings[0].pre_comment.as_deref(), Some("pre"));
        assert_eq!(
            doc.to_text(),
            "# top\r\nfoo=1 # inline\r\n\r\n[S] # sec\r\n# pre\r\nbar=2\r\n"
        );
    }

    #[test]
    fn quoted_values_keep_comment_chars() {
        let doc = Document::parse("a=\"x # y\"\r\n").unwrap();
        assert_eq!(doc.sections[0].settings[0].string_value(), "x # y");
        assert_eq!(doc.sections[0].settings[0].comment, None);
    }

    #[test]
    fn errors() {
        assert!(Document::parse("[oops\r\n").is_err());
        assert!(Document::parse("novalue\r\n").is_err());
        assert!(Document::parse("=1\r\n").is_err());
    }

    #[test]
    fn insert_and_remove() {
        let mut doc = Document::new();
        doc.section_or_insert("Client Settings")
            .get_or_insert("AutoConnect", "true");
        doc.section_or_insert("Client Settings")
            .get_or_insert("AutoConnect", "false");
        assert_eq!(doc.section("Client Settings").unwrap().settings.len(), 1);
        assert_eq!(doc.to_text(), "[Client Settings]\r\nAutoConnect=true\r\n");
        assert!(doc.section_mut("Client Settings").unwrap().remove("AutoConnect"));
        assert!(doc.remove_section("Client Settings"));
        assert_eq!(doc.to_text(), "");
    }
}
