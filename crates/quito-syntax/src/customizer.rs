//! Customizer schema extraction.
//!
//! OpenSCAD's Customizer turns top-level variable assignments into an editable
//! parameter UI, driven by comment annotations. This module reconstructs that
//! schema from source text (clean-room, from the public Customizer docs):
//!
//! * A top-level `name = <literal>;` is a parameter. Its control type is
//!   inferred from the literal (number → spinbox, `true/false` → checkbox,
//!   string → textbox, `[a,b,c]` of numbers → vector).
//! * A trailing `// [...]` annotation refines the control:
//!   - `// [min:max]` or `// [min:step:max]` → slider,
//!   - `// [a, b, c]` → dropdown (values); items may be `value:Label`,
//!   - a bare number after a string, `// 12`, → text max-length.
//! * A `//` comment on the line immediately above becomes the description
//!   (label); a trailing `// text` with no brackets is also a description.
//! * `/* [Group] */` starts a group; params fall under the most recent group
//!   (default group name is empty). The special group `[Hidden]` drops its
//!   params from the schema.
//!
//! This is a line scanner (comments are stripped by the lexer, and the
//! Customizer only concerns simple literal assignments), so it deliberately
//! ignores anything that isn't a plain top-level literal assignment.

/// A single customizable parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub group: String,
    pub description: Option<String>,
    pub value: ParamValue,
    pub control: Control,
}

/// A parameter's default value (a literal).
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Number(f64),
    Bool(bool),
    Text(String),
    Vector(Vec<f64>),
}

/// A dropdown option: an underlying value plus its display label.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub value: ParamValue,
    pub label: String,
}

/// The UI control a parameter should render as.
#[derive(Debug, Clone, PartialEq)]
pub enum Control {
    /// Free numeric entry.
    Number,
    /// Numeric slider (`[min:max]` / `[min:step:max]`).
    Slider { min: f64, step: Option<f64>, max: f64 },
    /// Boolean checkbox.
    Checkbox,
    /// Free text entry, optionally length-limited.
    Text { max_length: Option<u32> },
    /// A fixed set of choices.
    Dropdown(Vec<Choice>),
    /// A vector of numbers (component spinboxes).
    Vector { length: usize },
}

/// The extracted parameter schema, in source order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Customizer {
    pub params: Vec<Param>,
}

impl Customizer {
    /// True when the source exposes no customizable parameters.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }
}

/// Extract the customizer schema from OpenSCAD source.
pub fn extract(src: &str) -> Customizer {
    let mut params = Vec::new();
    let mut group = String::new();
    let mut hidden = false;
    let mut pending_desc: Option<String> = None;

    for raw in src.lines() {
        let line = raw.trim();

        // Group markers: /* [Group] */ (possibly with surrounding text).
        if let Some(g) = parse_group_marker(line) {
            hidden = g.eq_ignore_ascii_case("Hidden");
            group = if g.eq_ignore_ascii_case("Global") { String::new() } else { g };
            pending_desc = None;
            continue;
        }

        // A standalone line comment becomes the pending description.
        if let Some(rest) = line.strip_prefix("//") {
            let text = rest.trim();
            pending_desc = if text.is_empty() { None } else { Some(text.to_string()) };
            continue;
        }

        // Blank / non-assignment lines break the description association (a
        // description must sit on the line immediately above its parameter).
        let Some(assign) = parse_assignment(line) else {
            pending_desc = None;
            continue;
        };
        let (name, value_src, trailing) = assign;

        let Some(value) = parse_value(value_src) else {
            pending_desc = None;
            continue; // not a literal → not a customizer parameter
        };

        // A trailing comment is either a `[...]` control spec or a description.
        let mut description = pending_desc.take();
        let control = match trailing {
            Some(t) if t.trim_start().starts_with('[') => parse_control(t.trim(), &value),
            Some(t) => {
                let t = t.trim();
                // A bare number after a string sets max length.
                if let (ParamValue::Text(_), Ok(n)) = (&value, t.parse::<u32>()) {
                    Control::Text { max_length: Some(n) }
                } else {
                    if description.is_none() && !t.is_empty() {
                        description = Some(t.to_string());
                    }
                    default_control(&value)
                }
            }
            None => default_control(&value),
        };

        if !hidden {
            params.push(Param { name, group: group.clone(), description, value, control });
        }
    }

    Customizer { params }
}

/// `/* [Group] */` → `Some("Group")`. Accepts optional whitespace.
fn parse_group_marker(line: &str) -> Option<String> {
    let inner = line.strip_prefix("/*")?.strip_suffix("*/")?.trim();
    let g = inner.strip_prefix('[')?.strip_suffix(']')?.trim();
    Some(g.to_string())
}

/// Parse `name = value ;` optionally followed by `// trailing`. Returns
/// `(name, value_source, trailing_comment)`. Rejects anything that isn't a
/// plain identifier assignment.
fn parse_assignment(line: &str) -> Option<(String, &str, Option<&str>)> {
    let eq = line.find('=')?;
    // Reject ==, <=, >=, !=, => (not simple assignments).
    let after = line.as_bytes().get(eq + 1);
    if line.as_bytes().get(eq.wrapping_sub(1)).is_some_and(|b| matches!(b, b'!' | b'<' | b'>'))
        || after == Some(&b'=')
    {
        return None;
    }
    let name = line[..eq].trim();
    if name.is_empty() || !is_identifier(name) {
        return None;
    }
    let rest = &line[eq + 1..];
    // Split off a trailing line comment (outside of strings).
    let (value_part, trailing) = split_trailing_comment(rest);
    let value_part = value_part.trim();
    let value_src = value_part.strip_suffix(';').unwrap_or(value_part).trim();
    if value_src.is_empty() {
        return None;
    }
    Some((name.to_string(), value_src, trailing))
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c == '_' || c == '$' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Split `value ; // comment` into (`value ;`, `Some("comment")`), respecting
/// string literals so a `//` inside a string isn't treated as a comment.
fn split_trailing_comment(s: &str) -> (&str, Option<&str>) {
    let bytes = s.as_bytes();
    let mut in_str = false;
    let mut i = 0;
    while i + 1 <= bytes.len() {
        let c = bytes[i];
        if c == b'"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_str = !in_str;
        } else if !in_str && c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            return (&s[..i], Some(&s[i + 2..]));
        }
        i += 1;
    }
    (s, None)
}

fn default_control(v: &ParamValue) -> Control {
    match v {
        ParamValue::Bool(_) => Control::Checkbox,
        ParamValue::Number(_) => Control::Number,
        ParamValue::Text(_) => Control::Text { max_length: None },
        ParamValue::Vector(xs) => Control::Vector { length: xs.len() },
    }
}

/// Parse a literal value (`5`, `-2.5`, `true`, `"hi"`, `[1,2,3]`). Public so
/// callers (CLI `-D`, engine overrides) can parse a parameter value string.
pub fn parse_value(s: &str) -> Option<ParamValue> {
    let s = s.trim();
    if s == "true" {
        return Some(ParamValue::Bool(true));
    }
    if s == "false" {
        return Some(ParamValue::Bool(false));
    }
    if let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
        return Some(ParamValue::Text(unescape(inner)));
    }
    if let Some(inner) = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        let mut xs = Vec::new();
        for part in inner.split(',') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            xs.push(p.parse::<f64>().ok()?);
        }
        return Some(ParamValue::Vector(xs));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Some(ParamValue::Number(n));
    }
    None
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse a `[...]` control annotation given the parameter's value (used to
/// decide whether dropdown options are numeric or textual).
fn parse_control(annot: &str, value: &ParamValue) -> Control {
    let inner = match annot.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        Some(i) => i.trim(),
        None => return default_control(value),
    };

    // A range form has colons but no commas: min:max or min:step:max.
    if inner.contains(':') && !inner.contains(',') {
        let parts: Vec<&str> = inner.split(':').collect();
        let nums: Option<Vec<f64>> = parts.iter().map(|p| p.trim().parse::<f64>().ok()).collect();
        if let Some(nums) = nums {
            match nums.as_slice() {
                [min, max] => return Control::Slider { min: *min, step: None, max: *max },
                [min, step, max] => {
                    return Control::Slider { min: *min, step: Some(*step), max: *max }
                }
                _ => {}
            }
        }
        // Fall through: a single `label:value` with no comma is still a dropdown.
    }

    // Otherwise a comma-separated list of choices, each `value` or `value:label`.
    let numeric = matches!(value, ParamValue::Number(_));
    let mut choices = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (val_str, label) = match part.split_once(':') {
            Some((v, l)) => (v.trim(), l.trim().to_string()),
            None => (part, part.to_string()),
        };
        let value = if numeric {
            match val_str.parse::<f64>() {
                Ok(n) => ParamValue::Number(n),
                Err(_) => ParamValue::Text(val_str.to_string()),
            }
        } else {
            ParamValue::Text(unescape(val_str.trim_matches('"')))
        };
        choices.push(Choice { value, label });
    }
    if choices.is_empty() {
        default_control(value)
    } else {
        Control::Dropdown(choices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_names(src: &str) -> Vec<String> {
        extract(src).params.into_iter().map(|p| p.name).collect()
    }

    #[test]
    fn infers_control_from_value() {
        let c = extract("n = 5;\nflag = true;\ntxt = \"hi\";\nv = [1, 2, 3];\n");
        assert_eq!(c.params.len(), 4);
        assert_eq!(c.params[0].value, ParamValue::Number(5.0));
        assert_eq!(c.params[0].control, Control::Number);
        assert_eq!(c.params[1].control, Control::Checkbox);
        assert_eq!(c.params[2].control, Control::Text { max_length: None });
        assert_eq!(c.params[3].control, Control::Vector { length: 3 });
    }

    #[test]
    fn slider_ranges() {
        let c = extract("a = 5; // [0:10]\nb = 5; // [0:0.5:10]\n");
        assert_eq!(c.params[0].control, Control::Slider { min: 0.0, step: None, max: 10.0 });
        assert_eq!(
            c.params[1].control,
            Control::Slider { min: 0.0, step: Some(0.5), max: 10.0 }
        );
    }

    #[test]
    fn dropdowns_numeric_and_labeled() {
        let c = extract("x = 2; // [0, 1, 2, 3]\nmode = 1; // [0:Off, 1:On]\n");
        match &c.params[0].control {
            Control::Dropdown(opts) => {
                assert_eq!(opts.len(), 4);
                assert_eq!(opts[2].value, ParamValue::Number(2.0));
                assert_eq!(opts[2].label, "2");
            }
            other => panic!("{other:?}"),
        }
        match &c.params[1].control {
            Control::Dropdown(opts) => {
                assert_eq!(opts[0].value, ParamValue::Number(0.0));
                assert_eq!(opts[0].label, "Off");
                assert_eq!(opts[1].label, "On");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn string_dropdown_and_maxlength() {
        let c = extract("s = \"a\"; // [a, b, c]\nname = \"bob\"; // 8\n");
        match &c.params[0].control {
            Control::Dropdown(opts) => {
                assert_eq!(opts.len(), 3);
                assert_eq!(opts[1].value, ParamValue::Text("b".into()));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(c.params[1].control, Control::Text { max_length: Some(8) });
    }

    #[test]
    fn descriptions_and_groups() {
        let src = "\
/* [Box] */
// width of the box
width = 10; // [1:100]
height = 20;

/* [Hidden] */
secret = 42;

/* [Global] */
depth = 5; // depth here
";
        let c = extract(src);
        assert_eq!(extract_names(src), vec!["width", "height", "depth"]);
        assert_eq!(c.params[0].group, "Box");
        assert_eq!(c.params[0].description.as_deref(), Some("width of the box"));
        assert_eq!(c.params[1].group, "Box");
        assert_eq!(c.params[1].description, None);
        assert_eq!(c.params[2].group, ""); // Global → empty group
        assert_eq!(c.params[2].description.as_deref(), Some("depth here"));
    }

    #[test]
    fn ignores_non_literals_and_expressions() {
        // Expressions, comparisons, and module calls are not parameters.
        let src = "\
a = 1 + 2;
b == 3;
c = sin(30);
cube(a);
real = 7;
";
        assert_eq!(extract_names(src), vec!["real"]);
    }

    #[test]
    fn description_broken_by_blank_line() {
        let c = extract("// stale\n\nx = 1;\n");
        assert_eq!(c.params[0].description, None);
    }
}
