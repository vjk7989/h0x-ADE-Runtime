use std::io::{self, Write as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalNotificationBackend {
    Ghostty,
    Iterm2,
    Kitty,
    WezTerm,
}

pub fn detect_backend() -> Option<TerminalNotificationBackend> {
    let term_program = std::env::var("TERM_PROGRAM").ok();
    let term = std::env::var("TERM").ok();

    match term_program.as_deref() {
        Some("ghostty") => return Some(TerminalNotificationBackend::Ghostty),
        Some("iTerm.app") => return Some(TerminalNotificationBackend::Iterm2),
        Some("WezTerm") => return Some(TerminalNotificationBackend::WezTerm),
        _ => {}
    }

    if std::env::var_os("KITTY_WINDOW_ID").is_some() {
        return Some(TerminalNotificationBackend::Kitty);
    }

    match term.as_deref() {
        Some("xterm-ghostty") => Some(TerminalNotificationBackend::Ghostty),
        Some("xterm-kitty") => Some(TerminalNotificationBackend::Kitty),
        Some(term) if term.contains("wezterm") => Some(TerminalNotificationBackend::WezTerm),
        _ => None,
    }
}

pub fn show_notification(title: &str, body: Option<&str>) -> io::Result<bool> {
    let Some(backend) = detect_backend() else {
        return Ok(false);
    };

    let sequence = match backend {
        TerminalNotificationBackend::Ghostty
        | TerminalNotificationBackend::Iterm2
        | TerminalNotificationBackend::WezTerm => build_osc9_notification(title, body),
        TerminalNotificationBackend::Kitty => build_osc99_notification(title, body),
    };

    let sequence = if std::env::var_os("TMUX").is_some() {
        wrap_tmux_passthrough(&sequence)
    } else {
        sequence
    };

    let mut stdout = io::stdout();
    stdout.write_all(&sequence)?;
    stdout.flush()?;
    Ok(true)
}

pub fn split_message(message: &str) -> (&str, Option<&str>) {
    match message.split_once(": ") {
        Some((title, body)) if !title.is_empty() && !body.is_empty() => (title, Some(body)),
        _ => (message, None),
    }
}

fn build_osc9_notification(title: &str, body: Option<&str>) -> Vec<u8> {
    let message = sanitize_text(match body {
        Some(body) if !body.is_empty() => format!("{title}: {body}"),
        _ => title.to_string(),
    });
    format!("\x1b]9;{message}\x1b\\").into_bytes()
}

fn build_osc99_notification(title: &str, body: Option<&str>) -> Vec<u8> {
    let title = sanitize_text(title);
    match body {
        Some(body) if !body.is_empty() => {
            let body = sanitize_text(body);
            format!("\x1b]99;i=1:d=0;{title}\x1b\\\x1b]99;i=1:p=body;{body}\x1b\\").into_bytes()
        }
        _ => format!("\x1b]99;;{title}\x1b\\").into_bytes(),
    }
}

fn sanitize_text(text: impl AsRef<str>) -> String {
    text.as_ref()
        .chars()
        .map(|ch| match ch {
            '\n' | '\r' | '\t' => ' ',
            _ => ch,
        })
        .filter(|ch| !ch.is_control() && !crate::config::is_title_format_char(*ch))
        .collect()
}

fn wrap_tmux_passthrough(sequence: &[u8]) -> Vec<u8> {
    let mut wrapped = Vec::with_capacity(sequence.len() + 16);
    wrapped.extend_from_slice(b"\x1bPtmux;");
    for &byte in sequence {
        if byte == 0x1b {
            wrapped.push(0x1b);
        }
        wrapped.push(byte);
    }
    wrapped.extend_from_slice(b"\x1b\\");
    wrapped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_message_splits_title_and_body() {
        assert_eq!(
            split_message("agent done: ws · 1"),
            ("agent done", Some("ws · 1"))
        );
    }

    #[test]
    fn split_message_leaves_plain_message_alone() {
        assert_eq!(split_message("agent done"), ("agent done", None));
    }

    #[test]
    fn sanitize_text_strips_control_bytes() {
        assert_eq!(sanitize_text("a\n\tb\u{1b}c\u{7}"), "a  bc");
    }

    #[test]
    fn sanitize_text_strips_every_shared_format_range() {
        assert_eq!(
            sanitize_text("a\u{890}b\u{891}c\u{13430}d\u{1343f}e"),
            "abcde"
        );
    }

    #[test]
    fn sanitize_text_strips_c1_controls_and_format_characters() {
        assert_eq!(
            sanitize_text("a\u{9d}b\u{85}c\u{202e}d\u{200b}e\u{2066}f\u{7f}g"),
            "abcdefg"
        );
        let osc =
            String::from_utf8(build_osc9_notification("x\u{9c}y", Some("\u{202e}z"))).unwrap();
        assert_eq!(osc, "\x1b]9;xy: z\x1b\\");
    }

    #[test]
    fn sanitize_text_control_range_boundaries_and_joined_emoji() {
        assert_eq!(sanitize_text(""), "");
        // C1 is U+0080..=U+009F; NBSP just past it is printable and kept.
        assert_eq!(sanitize_text("a\u{80}b\u{9f}c\u{a0}d"), "abc\u{a0}d");
        // ZWJ and tag characters are format characters, so joined emoji fall apart.
        assert_eq!(
            sanitize_text("\u{1f469}\u{200d}\u{1f4bb} \u{1f3f4}\u{e0067}\u{e007f}"),
            "\u{1f469}\u{1f4bb} \u{1f3f4}"
        );
        assert_eq!(sanitize_text("\u{feff}\u{2028}\u{2029}\u{ad}"), "");
    }

    #[test]
    fn kitty_notification_strips_controls_from_title_and_body() {
        let sequence = String::from_utf8(build_osc99_notification(
            "t\u{9b}1\u{202e}",
            Some("b\u{1b}]\u{9c}\u{2066}"),
        ))
        .unwrap();
        assert_eq!(
            sequence,
            "\x1b]99;i=1:d=0;t1\x1b\\\x1b]99;i=1:p=body;b]\x1b\\"
        );
    }

    #[test]
    fn kitty_notification_uses_structured_title_and_body() {
        let sequence = String::from_utf8(build_osc99_notification("pi finished", Some("ws · 1")))
            .expect("utf8");
        assert!(sequence.contains("]99;i=1:d=0;pi finished"));
        assert!(sequence.contains("]99;i=1:p=body;ws · 1"));
    }

    #[test]
    fn tmux_passthrough_wraps_and_escapes() {
        let wrapped = wrap_tmux_passthrough(b"\x1b]9;hi\x1b\\");
        assert_eq!(wrapped, b"\x1bPtmux;\x1b\x1b]9;hi\x1b\x1b\\\x1b\\");
    }
}
