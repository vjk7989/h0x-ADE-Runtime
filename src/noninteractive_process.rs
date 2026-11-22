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
/// never runs. Windows: `%SystemRoot%/System32/curl.exe`; elsewhere `/usr/bin/curl`
/// when it exists, otherwise an absolute `curl` found on `PATH` (the installer's rule,
/// for NixOS, Termux and FreeBSD). Relative `PATH` entries are never used.
fn curl_program() -> std::path::PathBuf {
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:/Windows".into());
        std::path::PathBuf::from(root).join("System32/curl.exe")
    } else {
        unix_curl_program(
            std::path::Path::new("/usr/bin/curl"),
            std::env::var_os("PATH").as_deref(),
        )
    }
}

fn unix_curl_program(system: &std::path::Path, path_var: Option<&OsStr>) -> std::path::PathBuf {
    if system.exists() {
        return system.to_path_buf();
    }
    path_var
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("curl"))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| system.to_path_buf())
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
        } else if std::path::Path::new("/usr/bin/curl").exists() {
            assert_eq!(program, std::path::Path::new("/usr/bin/curl"));
        }
    }

    #[test]
    fn unix_curl_program_prefers_system_then_absolute_path_entries() {
        let dir = std::env::temp_dir().join(format!("h0x-curl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let system = dir.join("usr-bin-curl");
        let on_path = dir.join("curl");
        std::fs::write(&on_path, b"").unwrap();
        let path_var = std::env::join_paths([std::path::Path::new("relative"), &dir]).unwrap();
        // The system curl is missing: the absolute PATH entry wins, never the relative one.
        assert_eq!(unix_curl_program(&system, Some(&path_var)), on_path);
        // The system curl exists: PATH is ignored.
        std::fs::write(&system, b"").unwrap();
        assert_eq!(unix_curl_program(&system, Some(&path_var)), system);
        // Nothing found: the system path stays, and spawning reports "not found".
        std::fs::remove_file(&system).unwrap();
        let relative_only = std::env::join_paths(["relative"]).unwrap();
        assert_eq!(unix_curl_program(&system, Some(&relative_only)), system);
        assert_eq!(unix_curl_program(&system, None), system);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn curl_command_release_protocols_are_https_only() {
        // The release value is cfg-selected; this pins the source of truth for
        // non-test builds without compiling a second profile.
        let source = include_str!("noninteractive_process.rs");
        assert!(source.contains("const CURL_PROTOCOLS: &str = \"=https\";"));
    }
}
