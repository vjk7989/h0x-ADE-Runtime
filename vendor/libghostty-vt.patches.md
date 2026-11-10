# libghostty-vt local patches

This file tracks intentional local changes applied on top of the vendored
`libghostty-vt` source. Remove a patch only when the vendored source commit
contains the upstream behavior and the listed verification still passes.

## 0002 expose modifyOtherKeys mode through terminal data

status: active

patch: `vendor/patches/libghostty-vt/0002-expose-modify-other-keys-mode.patch`

herdr issue: none; fixes the performance regression exposed by
https://github.com/herdrdev/herdr/pull/2303

upstream discussion: not opened

upstream pr: not opened

vendored base: `83fb95624b0439ee660c2679b43b38c8b63769cd` (Ghostty v1.4.0)

local files:

- `vendor/libghostty-vt/include/ghostty/vt/terminal.h`
- `vendor/libghostty-vt/src/terminal/c/terminal.zig`

reason: Herdr must know whether xterm modifyOtherKeys mode 2 is active to
request printable key releases from the outer terminal. The formatter API can
recover this fact only by formatting the active screen and scrollback. A typed
terminal-data query exposes the authoritative scalar without formatting or
allocation. The local query uses value 43; upstream owns 33 (VT processing
errors) and, since v1.4.0, 41 (mouse shape) and 42 (memory usage).

remove when: the vendored source exposes an equivalent scalar query for
modifyOtherKeys mode 2 and Herdr can use it without this patch.

verification:

```sh
just test-one modify_other_keys
just test-one host_report_all_supplies_printable_releases_for_event_type_only_panes
just maintenance-test
just ui-hot-path-architecture-test
```

The former grapheme-default patch is replaced by upstream's public
`GHOSTTY_TERMINAL_OPT_MODE_DEFAULT` API. Herdr configures mode 2027 through that
API and tests that RIS restores it after a child disables it. The Wuffs C-only
mirror fix from Ghostty PR 13789 is also included in this vendored base.

## 0004 fix hosted Wuffs builds

status: active

patch: `vendor/patches/libghostty-vt/0004-fix-hosted-wuffs-builds.patch`

herdr issue: none; preserves Windows cross-compilation and non-SIMD hosted builds

upstream discussion: not opened

upstream pr: not opened

vendored base: `83fb95624b0439ee660c2679b43b38c8b63769cd` (Ghostty v1.4.0)

local files:

- `vendor/libghostty-vt/pkg/wuffs/src/main.zig`

reason: The libc forwarding half of this patch (Wuffs translate-c needing the
`--libc` file for Windows cross-builds) was dropped on the Zig 0.17 rebase:
`std.Build` no longer exposes `libc_file`, and the Zig 0.17 build maker applies
the `--libc` file to compile steps itself. Re-check Windows cross-compilation
(`LIBGHOSTTY_VT_WINDOWS_LIBC`) before relying on that.

The no-libc Wuffs module also exports hidden weak calloc/free stubs. On hosted
Linux with SIMD disabled, those definitions override the Rust executable's
libc allocator, causing immediate allocation failures. Limit the stubs to
freestanding targets; hosted embedders resolve these symbols through libc.

remove when: upstream prevents hosted allocator interposition and non-SIMD
native tests pass without this patch.

verification:

```sh
LIBGHOSTTY_VT_WINDOWS_LIBC=/path/to/windows-libc.txt just windows-lint
LIBGHOSTTY_VT_SIMD=false just test-one ghostty
just maintenance-test
```

## 0005 bounded word selection for wrapped link activation

status: active

patch: `vendor/patches/libghostty-vt/0005-bounded-word-selection.patch`

herdr issue: https://github.com/herdrdev/herdr/issues/1282

upstream discussion: not opened

upstream pr: not opened; related merged PR https://github.com/ghostty-org/ghostty/pull/10132
implements URL selection in the application layer, not the libghostty C API.

vendored base: `83fb95624b0439ee660c2679b43b38c8b63769cd` (Ghostty v1.4.0)

local files:

- `vendor/libghostty-vt/include/ghostty/vt/selection.h`
- `vendor/libghostty-vt/src/lib_vt.zig`
- `vendor/libghostty-vt/src/terminal/Screen.zig`
- `vendor/libghostty-vt/src/terminal/c/main.zig`
- `vendor/libghostty-vt/src/terminal/c/selection.zig`

reason: Ctrl+click must resolve a wrapped token beyond the visible viewport
without scanning an arbitrarily long logical line. The new, opt-in API shares
one cell-inspection budget across both directions and returns no selection on
exhaustion, never a truncated link. Its scan skips wide-character spacer cells.
The existing word-selection functions and option layouts remain unchanged;
only Herdr's link activation uses the new function.

remove when: upstream provides an equivalent bounded, wrap-aware selection API
that handles wide-character spacers, and Herdr passes the tests below using it
without this patch.

verification:

```sh
just test-one link_target
just test-one link_activation
just test-one ctrl_click
just check
```

## 0006 clear screen while preserving the cursor line

status: active

patch: `vendor/patches/libghostty-vt/0006-clear-screen-preserving-cursor-line.patch`

herdr issue: none; requested in https://github.com/herdrdev/herdr/discussions/545

upstream discussion: not opened

upstream pr: not opened

vendored base: `83fb95624b0439ee660c2679b43b38c8b63769cd` (Ghostty v1.4.0)

local files:

- `vendor/libghostty-vt/include/ghostty/vt/terminal.h`
- `vendor/libghostty-vt/src/lib_vt.zig`
- `vendor/libghostty-vt/src/terminal/c/main.zig`
- `vendor/libghostty-vt/src/terminal/c/terminal.zig`

reason: Herdr needs an explicit screen/history clear that preserves the cursor's
visible soft-wrapped line without writing to the child or interrupting a partial
VT sequence. The new C function operates directly on the screen, leaves alternate
screens untouched, clears image placements, and marks the result dirty.

remove when: the vendored C API provides an equivalent parser-independent clear
operation preserving the visible cursor line, and Herdr passes the checks below
using it without this patch.

verification:

```sh
just test-one clear_pane
just maintenance-test
just check
```

## 0007 experimental encoded PNG and immutable source retention

status: active

patch: `vendor/patches/libghostty-vt/0007-experimental-png-retention.patch`

herdr issue: none; maintainer-directed native Kitty forwarding experiment

upstream discussion: not opened

upstream pr: not opened

vendored base: `83fb95624b0439ee660c2679b43b38c8b63769cd` (Ghostty v1.4.0)

local files:

- `vendor/libghostty-vt/include/ghostty/vt/terminal.h`
- `vendor/libghostty-vt/include/ghostty/vt/kitty_graphics.h`
- `vendor/libghostty-vt/src/terminal/c/terminal.zig`
- `vendor/libghostty-vt/src/terminal/c/kitty_graphics.zig`
- `vendor/libghostty-vt/src/terminal/kitty/graphics_image.zig`
- `vendor/libghostty-vt/src/terminal/kitty/graphics_exec.zig`
- `vendor/libghostty-vt/src/terminal/kitty/graphics_storage.zig`

reason: An explicitly enabled embedding mode retains structurally validated,
quiet PNG uploads as encoded bytes, avoiding pixel decoding before forwarding
through a multiplexer. The existing raw getters retain their meaning; separate
getters expose retained PNG bytes. Queries and response-bearing uploads retain
full validation, and animation operations materialize pixels transactionally.
Storage reserves both encoded bytes and expected decoded size.

This is default-off and experimental: CRC-valid corrupt compressed pixels may
be rejected later than in normal mode, including after placement. Quiet mode
suppresses replies, not validation semantics; this patch is not a claim of full
protocol-equivalent transparent forwarding. Herdr exercises this mode only in
tests; production PNG uploads retain full decoding and validation.

A separate default-off snapshot callback retains host-owned immutable raw RGBA
file backing before reading pixels. Herdr installs this callback automatically
on Linux, using same-filesystem CoW snapshots. It never retains a mutable producer pathname. Unsupported snapshots
use the original loader; animation materializes pixels transactionally. Backing
ownership and bounded reads are explicit in the embedding ABI.

remove when: upstream provides an equivalent opt-in owned encoded-image
representation and immutable host-backed raw sources with bounded storage,
strict query handling and lazy pixel materialization, or this experiment is retired.

verification:

```sh
just test-one native_source
just test-one png_forward_tests
just test-one kitty_png_replacement
just test-one kitty_file_image_survives
(cd vendor/libghostty-vt && zig build test-lib-vt -Dtest-filter='experimental PNG')
just check
```

## 0008 keep cursor hyperlink across page replacement

status: active

patch: `vendor/patches/libghostty-vt/0008-cursor-hyperlink-page-replace.patch`

herdr issue: https://github.com/herdrdev/herdr/issues/5296

upstream discussion: not opened

upstream pr: not opened

vendored base: `83fb95624b0439ee660c2679b43b38c8b63769cd` (Ghostty v1.4.0)

local files:

- `vendor/libghostty-vt/src/terminal/Screen.zig`

reason: Moving the cursor onto another page while a hyperlink and a
non-default style are active can grow that page. Growth replaces the page,
re-adds the same heap hyperlink, and frees it. The outer move then clears
the pointer without clearing the id that replacement just installed, so
`endHyperlink` dereferences null and the server dies. The same sequence is
still present on upstream main (`cursorChangePin` around line 1606 and
`cursorPageReplaced` around line 706 at 22fa134a0). This is not the one-row
scrollback prune fixed by 65ff54c67.

The same function also crashes when style migration hits the maximum style
capacity: the page split moves the cursor again, and the nested pin change
releases hyperlink id 0 from a page that no longer holds it. The patch skips
that release when an outer pin change already released the link.

remove when: the vendored source keeps the cursor hyperlink alive across a
page replacement or page split during a pin change, and both Screen tests
below pass without this patch.

verification:

```sh
(cd vendor/libghostty-vt && zig build test-lib-vt -Dtest-filter='cursorAbsolute across a full page keeps the hyperlink')
(cd vendor/libghostty-vt && zig build test-lib-vt -Dtest-filter='cursorAbsolute keeps the hyperlink when style migration splits the page')
just maintenance-test
```

## Rebase onto Ghostty v1.4.0 (2026-11-10)

The vendored base moved from `44f2a44df7` to Ghostty v1.4.0
(`83fb95624b0439ee660c2679b43b38c8b63769cd`, requires Zig 0.17.0). It carries the
four 2026-10-27 security fixes (GHSA-w9q4-5jvv-mcv4, GHSA-qjfx-r533-6vx3,
GHSA-jqr7-69mm-v35h, GHSA-3r5j-c6rj-xq2v). The tarball is `zig build dist
-Demit-lib-vt` output from a clean v1.4.0 checkout; `zig-pkg/` is no longer part
of it (packages are fetched into the Zig global cache at build time). All six
patches were re-derived on the new base:

- 0002: data value 41 is taken upstream (mouse shape), so
  `GHOSTTY_TERMINAL_DATA_MODIFY_OTHER_KEYS` is now 43.
- 0004: only the `main.zig` half remains. The `build.zig` libc forwarding was
  dropped because `std.Build.libc_file` no longer exists in Zig 0.17.
- 0005: upstream's `selectWord` now follows wide-character spacers through
  `selectWordCodepoint`; `selectWordBounded` mirrors that logic with the same
  shared cell budget.
- 0006, 0008: unchanged in behaviour.
- 0007: option values 40 to 47 are taken upstream, so
  `GHOSTTY_TERMINAL_OPT_KITTY_IMAGE_PRESERVE_PNG` is 48 and
  `GHOSTTY_TERMINAL_OPT_KITTY_IMAGE_SNAPSHOT_FILE` is 49.

`crates/ghostty-vt/src/bindings.rs` was regenerated from the new headers.
