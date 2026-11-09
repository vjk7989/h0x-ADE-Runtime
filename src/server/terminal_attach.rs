pub(crate) fn paste_payload_for_runtime(
    runtime: &crate::terminal::TerminalRuntime,
    text: &str,
) -> String {
    if runtime.bracketed_paste_enabled() {
        let text = crate::pane::strip_paste_escapes(text);
        format!("\x1b[200~{text}\x1b[201~")
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::paste_payload_for_runtime;

    #[tokio::test]
    async fn bracketed_paste_payload_drops_esc_and_c1_from_the_text() {
        let (runtime, _rx) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
        // Not bracketed: text is passed through untouched.
        assert_eq!(paste_payload_for_runtime(&runtime, "a\x1bb"), "a\x1bb");
        runtime.test_process_pty_bytes(b"\x1b[?2004h");
        assert_eq!(
            paste_payload_for_runtime(&runtime, "/tmp/x\x1b[201~;rm\u{9b}201~"),
            "\x1b[200~/tmp/x[201~;rm201~\x1b[201~"
        );
        assert_eq!(
            paste_payload_for_runtime(&runtime, ""),
            "\x1b[200~\x1b[201~"
        );
    }
}
