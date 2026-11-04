use std::{ffi::OsStr, process::Command};

/// Protocols curl may use for the initial request.
#[cfg(not(test))]
const CURL_PROTOCOLS: &str = "=https";
/// Unit tests serve fixtures from `file://` URLs.
#[cfg(test)]
const CURL_PROTOCOLS: &str = "=https,file";

/// Builds a subprocess whose stdio is controlled by the caller and which never opens a Windows console.
pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    crate::platform::configure_background_command(&mut command);
    command
}

/// curl restricted to HTTPS for the request and every redirect.
pub(crate) fn curl_command() -> Command {
    let mut command = command("curl");
    command.args(["--proto", CURL_PROTOCOLS, "--proto-redir", "=https"]);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curl_command_restricts_protocols_before_caller_args() {
        let mut command = curl_command();
        command.arg("https://example.com/");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            [
                "--proto",
                "=https,file",
                "--proto-redir",
                "=https",
                "https://example.com/"
            ]
        );
    }

    #[test]
    fn curl_command_release_protocols_are_https_only() {
        // The release value is cfg-selected; this pins the source of truth for
        // non-test builds without compiling a second profile.
        let source = include_str!("noninteractive_process.rs");
        assert!(source.contains("const CURL_PROTOCOLS: &str = \"=https\";"));
    }
}
