use super::*;

pub(super) fn decode_clipboard_payload(data: &str) -> Option<Vec<u8>> {
    use base64::Engine;

    base64::engine::general_purpose::STANDARD.decode(data).ok()
}

/// Forward a pane's OSC 52 write to the system clipboard, only when
/// `terminal.clipboard_write` allows it. Blocked writes touch nothing.
pub(super) fn forward_clipboard(data: &str, allowed: bool) -> bool {
    if !allowed {
        debug!("blocked OSC 52 clipboard write: terminal.clipboard_write is off");
        return false;
    }
    let Some(bytes) = decode_clipboard_payload(data) else {
        warn!("received invalid clipboard payload from server");
        return false;
    };
    crate::selection::write_osc52_bytes(&bytes)
}
