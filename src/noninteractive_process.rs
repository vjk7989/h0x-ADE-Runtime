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

/// The system curl by absolute path, so a planted `curl` earlier on `PATH`
/// never runs. Windows: `%SystemRoot%/System32/curl.exe`; elsewhere
/// `/usr/bin/curl` (spawning fails with "not found" when it is missing).
fn curl_program() -> std::path::PathBuf {
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:/Windows".into());
        std::path::PathBuf::from(root).join("System32/curl.exe")
    } else {
        std::path::PathBuf::from("/usr/bin/curl")
    }
}

/// curl restricted to HTTPS for the request and every redirect.
pub(crate) fn curl_command() -> Command {
    let mut command = command(curl_program());
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
    fn curl_command_uses_an_absolute_system_path() {
        let command = curl_command();
        let program = std::path::Path::new(command.get_program());
        assert!(program.is_absolute(), "{program:?}");
        if cfg!(windows) {
            assert!(program.ends_with("System32/curl.exe"), "{program:?}");
        } else {
            assert_eq!(program, std::path::Path::new("/usr/bin/curl"));
        }
    }

    #[test]
    fn curl_command_release_protocols_are_https_only() {
        // The release value is cfg-selected; this pins the source of truth for
        // non-test builds without compiling a second profile.
        let source = include_str!("noninteractive_process.rs");
        assert!(source.contains("const CURL_PROTOCOLS: &str = \"=https\";"));
    }
}
