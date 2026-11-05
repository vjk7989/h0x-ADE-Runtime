# Changelog

## Unreleased

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
