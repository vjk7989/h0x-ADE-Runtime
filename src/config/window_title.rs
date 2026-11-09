/// Maximum characters in a rendered outer terminal window title.
pub(crate) const MAX_WINDOW_TITLE_CHARS: usize = 200;

pub(crate) fn default_window_title() -> String {
    "{hostname}: {workspace}".to_string()
}

/// Unicode format characters (category Cf) plus the line/paragraph separators:
/// bidi overrides and isolates, zero-width marks, BOM, tag characters.
pub(crate) fn is_title_format_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{ad}'
            | '\u{600}'..='\u{605}'
            | '\u{61c}'
            | '\u{6dd}'
            | '\u{70f}'
            | '\u{8e2}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}

pub(crate) fn sanitize_window_title_text(value: &str) -> Option<String> {
    let filtered = value
        .chars()
        .filter(|ch| !matches!(*ch, '\u{1b}' | '\u{7}' | '\u{9c}') && !ch.is_control())
        .filter(|ch| !is_title_format_char(*ch))
        .collect::<String>();
    let sanitized = filtered
        .trim()
        .chars()
        .take(MAX_WINDOW_TITLE_CHARS)
        .collect::<String>()
        .trim_end()
        .to_string();
    (!sanitized.is_empty()).then_some(sanitized)
}

/// A value the server can substitute into `ui.window_title`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowTitleToken {
    /// Host the HyperOx server runs on.
    Hostname,
    /// Active workspace display name.
    Workspace,
    /// Active tab display name.
    Tab,
    /// Manual name of the focused pane.
    Pane,
    /// Focused pane's terminal title, with spinner frames stripped.
    TerminalTitle,
}

impl WindowTitleToken {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "hostname" => Some(Self::Hostname),
            "workspace" => Some(Self::Workspace),
            "tab" => Some(Self::Tab),
            "pane" => Some(Self::Pane),
            "terminal_title" => Some(Self::TerminalTitle),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowTitlePart {
    Literal(String),
    Token(WindowTitleToken),
}

/// Parsed `ui.window_title` format string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowTitleTemplate {
    parts: Vec<WindowTitlePart>,
}

impl WindowTitleTemplate {
    /// Parses a window title format. An empty format disables outer window
    /// titles and parses to `Ok(None)`.
    pub fn parse(template: &str) -> Result<Option<Self>, String> {
        if template.is_empty() {
            return Ok(None);
        }

        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut chars = template.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    literal.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    literal.push('}');
                }
                '{' => {
                    let mut name = String::new();
                    let mut closed = false;
                    for ch in chars.by_ref() {
                        if ch == '}' {
                            closed = true;
                            break;
                        }
                        name.push(ch);
                    }
                    if !closed {
                        return Err("has an unclosed '{'".to_string());
                    }
                    let Some(token) = WindowTitleToken::parse(name.trim()) else {
                        return Err(format!("has unknown token '{{{name}}}'"));
                    };
                    if !literal.is_empty() {
                        parts.push(WindowTitlePart::Literal(std::mem::take(&mut literal)));
                    }
                    parts.push(WindowTitlePart::Token(token));
                }
                '}' => return Err("has an unmatched '}'".to_string()),
                _ => literal.push(ch),
            }
        }
        if !literal.is_empty() {
            parts.push(WindowTitlePart::Literal(literal));
        }

        Ok((!parts.is_empty()).then_some(Self { parts }))
    }

    pub fn parts(&self) -> &[WindowTitlePart] {
        &self.parts
    }

    pub fn uses(&self, token: WindowTitleToken) -> bool {
        self.parts
            .iter()
            .any(|part| matches!(part, WindowTitlePart::Token(other) if *other == token))
    }
}

pub(crate) fn window_title_diagnostics(template: &str) -> Option<String> {
    WindowTitleTemplate::parse(template)
        .err()
        .map(|err| format!("ui.window_title {err}; leaving the outer terminal title alone"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tokens_literals_and_escapes() {
        let template = WindowTitleTemplate::parse("{hostname}: {workspace} {{a}}")
            .expect("parse")
            .expect("template");

        assert_eq!(
            template.parts(),
            [
                WindowTitlePart::Token(WindowTitleToken::Hostname),
                WindowTitlePart::Literal(": ".into()),
                WindowTitlePart::Token(WindowTitleToken::Workspace),
                WindowTitlePart::Literal(" {a}".into()),
            ]
        );
        assert!(template.uses(WindowTitleToken::Hostname));
        assert!(!template.uses(WindowTitleToken::TerminalTitle));
    }

    #[test]
    fn default_template_parses() {
        assert!(WindowTitleTemplate::parse(&default_window_title())
            .expect("parse")
            .is_some());
    }

    #[test]
    fn empty_template_disables_window_titles() {
        assert_eq!(WindowTitleTemplate::parse(""), Ok(None));
        assert!(window_title_diagnostics("").is_none());
    }

    #[test]
    fn invalid_templates_report_diagnostics() {
        assert!(window_title_diagnostics("{hostname")
            .expect("diagnostic")
            .contains("unclosed"));
        assert!(window_title_diagnostics("a } b")
            .expect("diagnostic")
            .contains("unmatched"));
        assert!(window_title_diagnostics("{session}")
            .expect("diagnostic")
            .contains("unknown token '{session}'"));
    }

    #[test]
    fn sanitizes_and_bounds_rendered_titles() {
        assert_eq!(
            sanitize_window_title_text("  h0x\u{1b} api\u{7}\n  ").as_deref(),
            Some("h0x api")
        );
        assert_eq!(sanitize_window_title_text("\u{7}\n"), None);
        assert_eq!(
            sanitize_window_title_text(&"x".repeat(MAX_WINDOW_TITLE_CHARS + 1))
                .expect("title")
                .chars()
                .count(),
            MAX_WINDOW_TITLE_CHARS
        );
    }

    #[test]
    fn token_names_tolerate_surrounding_whitespace() {
        let template = WindowTitleTemplate::parse("{ tab }")
            .expect("parse")
            .expect("template");

        assert_eq!(
            template.parts(),
            [WindowTitlePart::Token(WindowTitleToken::Tab)]
        );
    }

    #[test]
    fn sanitize_drops_format_characters_and_trims_before_the_cap() {
        // Bidi overrides, isolates, marks, zero-width, BOM, tags, LS/PS.
        assert_eq!(
            sanitize_window_title_text("a\u{202e}b\u{2066}c\u{2069}d\u{200e}e\u{200f}f\u{2028}g\u{2029}h\u{feff}i\u{200b}j\u{e0041}k"),
            Some("abcdefghijk".to_string())
        );
        assert_eq!(sanitize_window_title_text("\u{202e}\u{200b}\u{2028}"), None);
        // Ordinary non-ASCII text survives.
        assert_eq!(
            sanitize_window_title_text("日本 é \u{a0}x"),
            Some("日本 é \u{a0}x".to_string())
        );
        // Leading whitespace does not eat into the cap: trim happens first.
        let padded = format!("{}{}", " ".repeat(50), "x".repeat(MAX_WINDOW_TITLE_CHARS));
        assert_eq!(
            sanitize_window_title_text(&padded),
            Some("x".repeat(MAX_WINDOW_TITLE_CHARS))
        );
        // Format chars removed before trim: they cannot hide padding either.
        let hidden = format!("\u{200b}  {}", "y".repeat(MAX_WINDOW_TITLE_CHARS + 5));
        assert_eq!(
            sanitize_window_title_text(&hidden),
            Some("y".repeat(MAX_WINDOW_TITLE_CHARS))
        );
    }
}
