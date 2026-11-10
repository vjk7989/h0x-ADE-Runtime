# Changelog

## Unreleased

## [0.9.15] - 2026-11-11

Security release from the final QA round.

### Fixed
- Built on Ghostty 1.4.0 (Zig 0.17), which fixes four terminal-engine advisories (GHSA-w9q4-5jvv-mcv4, GHSA-qjfx-r533-6vx3, GHSA-jqr7-69mm-v35h, GHSA-3r5j-c6rj-xq2v).
- A remote server can no longer send control characters or oversized cells to your terminal; each remote cell holds one character at most two columns wide.
- Terminal sizes requested by clients are capped at 1000×1000.
- A forwarded SSH agent stays tied to its connection after a server restart.
- The installers check the release signature with your installed h0x before replacing anything; plain http is allowed only for 127.0.0.1 and [::1]. New `h0x update --verify <file>`.
- curl is run from its system path.

## [0.9.14] - 2026-11-10

0.9.13 was tagged but never published: its macOS release builds stalled on a Zig package download. 0.9.14 contains everything listed under 0.9.13.

### Changed
- Release and CI builds download Zig packages up front with a time limit and retries, and cache them.

## [0.9.13] - 2026-11-10

### Changed
- Release builds use thin link-time optimization, a single code-generation unit and stripped symbols, for smaller and faster binaries.

## [0.9.12] - 2026-11-09

Dependency security release.

### Fixed
- Updated anyhow, rand, tokio and lru (via ratatui-core 0.1.2) past RUSTSEC-2026-0190, RUSTSEC-2026-0097, RUSTSEC-2026-0386 and RUSTSEC-2026-0253.

### Changed
- Halfwidth katakana with voiced marks (for example ｶﾞ) now take 2 columns in the sidebar, tabs and overlays, matching how the terminal draws them.

## [0.9.11] - 2026-11-09

Security release from the second audit (Cloudflare security-audit tool).

### Fixed
- A remote server can no longer write raw terminal bytes to your screen, including kitty image commands split across frames or hidden in surface graphics; image decoding and remote responses have size limits.
- Agent resume commands are quoted for the pane's shell (PowerShell included), and session ids starting with `-` are refused.
- The git status reader ignores repositories owned by another account and never follows network paths.
- The plugin confirmation's 5-minute block after Deny applies per program, and a remote server's prompt shows that server's name.
- Notification text loses control and hidden formatting characters; Linux notification bodies are escaped.
- The notification sound is written to a private temporary folder.

## [0.9.10] - 2026-11-09

Security release from a full audit.

### Changed
- Programs in panes can no longer write to the system clipboard (OSC 52) unless you turn it on with `[terminal] clipboard_write = true`.
- Approving a plugin or integration now needs Tab (to "Allow") and then Enter; the prompt ignores input for its first second, shows which program asked, and refuses the same program for 5 minutes after a Deny.
- Kitty graphics no longer load images from local files; images sent inline still work (`kitten icat` file mode is off).
- The agent-detection catalog is downloaded as a signed release file and checked before use.
- Updates and remote installs download only from GitHub, over https.

### Fixed
- Pasted text can no longer leave bracketed-paste mode.
- Window titles from remote servers lose control and hidden formatting characters.
- PowerShell commands quote paths with curly quotes correctly.
- Windows: a local connection is only trusted if the other side runs as you at the same or a higher integrity level.
- Docs: plugin example links point to the real upstream examples; agent-skill links use the `main` branch.

## [0.9.9] - 2026-11-05

### Changed
- The sidebar shows rotating HyperOx busy words while an agent works.
- Docs: plugin examples use a neutral `owner/h0x-plugin`, and troubleshooting links point to our issues.
- Tests keep plugin state in a temporary home.

## [0.9.8] - 2026-11-05

### Changed
- New logo: the HyperOx mark on a white rounded tile, so it shows on dark backgrounds.
- On Windows the ConPTY override is now `H0X_WINDOWS_CONPTY=system` (was `HERDR_WINDOWS_CONPTY`), and the bundle marker is `h0x-conpty.json`.
- Troubleshooting links now say "upstream issue" for issues from the original project.

## [0.9.7] - 2026-11-05

### Fixed
- On Windows, h0x refuses to talk to a pipe run by another account.
- Folders reported by pane output on network shares or device paths are ignored, so terminal output can't make Windows contact a network share.
- Git status ignores malformed refs, so repository files can't inject git options.
- Release files must be signed for that exact file.

## [0.9.6] - 2026-11-05

### Changed
- Updates are signed. `h0x update` checks the release's minisign signature on the manifest and on the downloaded binary, and refuses an update that is unsigned or does not match the release key.
- Installing h0x on a remote machine checks the same signature before the binary is used.
- Agent hook installs and plugin links or enables requested through the socket API ask in the attached client first. The request waits for Allow or Deny, gives up after 120 seconds, and fails with `needs_confirmation` when no client is attached. The `h0x integration install`, `h0x plugin link` and `h0x plugin enable` commands still run without a prompt outside any pane on the local machine; inside a pane or with `--machine` they ask.
- Downloads use HTTPS only. The installers accept plain http only for loopback addresses.

### Fixed
- Release notes and product announcements no longer pass terminal control characters through to your terminal.
- Session files are private to your user (files 0600, folders 0700).
- Git refs and branch names that start with `-` are refused, so they cannot be read as git options.

## [0.9.5] - 2026-11-04

### New
- First HyperOx runtime release.
