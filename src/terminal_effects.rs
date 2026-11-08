use std::io::{self, Write};

const BELL_CHUNK: [u8; 64] = [b'\x07'; 64];

pub(crate) fn write_terminal_bells<W: Write>(writer: &mut W, count: u16) -> io::Result<()> {
    let full_chunks = usize::from(count) / BELL_CHUNK.len();
    let remainder = usize::from(count) % BELL_CHUNK.len();
    for _ in 0..full_chunks {
        writer.write_all(&BELL_CHUNK)?;
    }
    writer.write_all(&BELL_CHUNK[..remainder])?;
    writer.flush()
}

pub(crate) fn write_window_title<W: Write>(writer: &mut W, title: Option<&str>) -> io::Result<()> {
    // A remote server's title is untrusted: same sanitizer the server applies.
    let safe_title = title
        .and_then(crate::config::sanitize_window_title_text)
        .unwrap_or_else(|| "h0x".to_string());
    write!(writer, "\x1b]0;{safe_title}\x07")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_exact_terminal_bell_count() {
        let mut output = Vec::new();

        write_terminal_bells(&mut output, 130).unwrap();

        assert_eq!(output, vec![b'\x07'; 130]);
    }

    #[test]
    fn window_title_strips_terminators_and_defaults_to_h0x() {
        let mut output = Vec::new();
        write_window_title(&mut output, Some("h0x\x1b api\u{7}\u{9c}")).unwrap();
        assert_eq!(output, b"\x1b]0;h0x api\x07");

        output.clear();
        write_window_title(&mut output, None).unwrap();
        assert_eq!(output, b"\x1b]0;h0x\x07");
    }

    #[test]
    fn window_title_drops_every_control_character() {
        let mut output = Vec::new();
        write_window_title(
            &mut output,
            Some("a\r\nb\u{18}c\u{1a}d\u{85}e\u{9b}f\u{7f}g\x1b]52;c;x\x07"),
        )
        .unwrap();
        assert_eq!(output, b"\x1b]0;abcdefg]52;c;x\x07");

        // A title made only of controls falls back to the default.
        output.clear();
        write_window_title(&mut output, Some("\r\n\u{18}\u{9b}")).unwrap();
        assert_eq!(output, b"\x1b]0;h0x\x07");
        output.clear();
        write_window_title(&mut output, Some("")).unwrap();
        assert_eq!(output, b"\x1b]0;h0x\x07");
    }
}
