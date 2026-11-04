# HyperOx agent guide

Use this guide to help a human understand, set up, or troubleshoot HyperOx. It covers HyperOx's concept model, setup path, and diagnosis recipes. Canonical documentation lives at https://github.com/vjk7989/h0x-ADE-Runtime/tree/main/docs/next/website/src/content/docs. Point the human there for more detail, and verify any command you are unsure about against those pages instead of guessing.

If you are running *inside* a HyperOx pane (the environment variable `H0X_ENV=1` is set), HyperOx also ships a skill file that teaches you to control HyperOx through the `h0x` CLI: https://raw.githubusercontent.com/vjk7989/h0x-ADE-Runtime/main/skills/h0x/SKILL.md. That file teaches you to operate HyperOx; this one teaches you to guide a human.

## What HyperOx is

HyperOx is a terminal workspace manager for AI coding agents. Like tmux, it is a multiplexer: a background server owns real terminal processes, and clients attach to render them. Panes keep running when the human detaches, closes the terminal, or disconnects SSH.

Unlike tmux, HyperOx is mouse-first and agent-aware. The whole UI is clickable — panes, tabs, workspaces, split borders, right-click menus. HyperOx detects coding agents running inside panes and shows each one's state in a sidebar, so the human can see across all their projects which agent is `working`, which is `blocked` waiting for input, and which is `done`. A CLI and a local socket API let scripts and agents drive HyperOx programmatically.

## Concept model

Teach these in this order:

- **Session** — a persistent background server namespace. Running `h0x` attaches to the default session. Named sessions (`h0x session attach work`) are fully separate runtime namespaces; most people only need the default.
- **Workspace** — the project-level container. One per repo, task, or investigation. Owns tabs and panes. The sidebar rolls agent states up per workspace.
- **Tab** — a layout inside a workspace, for separating views like `agents`, `logs`, `server`.
- **Pane** — a real terminal. Splittable right or down. Survives client detach.
- **Agent** — a process HyperOx recognizes inside a pane. States: `working`, `blocked`, `done`, `idle`, `unknown`.
- **Modes** — terminal mode sends keys to the focused pane; prefix mode (`ctrl+b`, then one action key) sends one command to HyperOx; navigate mode is a persistent navigation surface.

Full concepts page: https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/concepts.mdx

## Install

Linux and macOS:

```bash
curl -fsSL https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest/download/install.sh | sh
h0x
```

Windows PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest/download/install.ps1 | iex"
h0x
```

If endpoint security blocks that fileless PowerShell command, use Command Prompt:

```cmd
curl.exe -fsSLo install.cmd https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest/download/install.cmd && install.cmd && del install.cmd
h0x
```

Homebrew, mise, and Nix installs, verification, and manual downloads: https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/install.mdx. Direct installs use the stable channel by default and update with `h0x update`; preview is opt-in. Package-manager installs update through that package manager. Check the version with `h0x --version`.

## First-run walkthrough

Check your environment first. If `H0X_ENV=1` is set, you are already running inside a HyperOx pane. The human is already attached, so skip step 1 and never tell them to run `h0x` from your pane; HyperOx blocks nested launches by design. Start with step 2, and consider the skill file below.

Walk the human through this sequence:

1. `cd` into a project and run `h0x`. It launches or attaches to the default background session and creates a workspace automatically. First run shows an onboarding flow.
2. Start their coding agent in the pane — `claude`, `codex`, or any supported agent (full list: https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/agents.mdx). HyperOx detects it automatically; the sidebar shows its state. Install the matching integration when available. Depending on the agent, it provides lifecycle state, native session restore, or both. For example, `h0x integration install claude` adds native session restore, while Claude's state still comes from screen detection.
3. Start with the mouse: click panes and tabs to focus, drag split borders, right-click for menus, drag-select to copy. No keybindings are required to use HyperOx.
4. Split panes: right-click menu, or `prefix+v` (right) / `prefix+minus` (down). New tab: `prefix+c`.
5. Detach with `prefix+q` (press `ctrl+b`, release, press `q`) or close the terminal window. Everything keeps running. Reattach later with `h0x`.
6. To actually stop everything: `h0x server stop`.

## The keyboard story

New users do not need to learn keybindings; the mouse covers everything. When the human wants keyboard control:

- The prefix key is `ctrl+b` by default. `prefix+?` shows every active binding live.
- The guided keyboard page covers the prefix, the bindings to learn first, and a vetted prefix-free setup using `ctrl+alt` chords: https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/keyboard.mdx. Recommend it over improvising.
- Every binding, including the prefix itself, is configurable under `[keys]` in the config file.
- If a direct chord does nothing, the OS or the outer terminal consumed it before HyperOx could see it. The keyboard page explains which chords are safe and why.

## Install the HyperOx skill into yourself

HyperOx ships `skills/h0x/SKILL.md` (https://raw.githubusercontent.com/vjk7989/h0x-ADE-Runtime/main/skills/h0x/SKILL.md), which teaches a coding agent to control HyperOx from inside a pane: splitting panes, running commands without stealing focus, reading output, and waiting on other agents.

Once the human is set up, offer to install it for your coding agent so future sessions can control HyperOx directly. For agents supported by the open skills CLI, use `npx skills add vjk7989/h0x-ADE-Runtime --skill h0x -g`. For agents without a skill system, add the GitHub copy above to their global custom instructions. Ask the human before writing to their config locations, and use the GitHub copy above as the source of truth.

## Configuration

- Config file: `~/.config/h0x/config.toml` on Linux and macOS; `%APPDATA%\h0x\config.toml` on Windows. HyperOx works without one.
- Print the full default config: `h0x --default-config`.
- Apply edits to a running server: `h0x server reload-config` (or the global menu → reload config).
- Main areas: `[keys]` keybindings, `[theme]` themes, `[ui]` sidebar and UI behavior, `[terminal]` shell defaults, `[update]` channel.
- Full reference: https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/configuration.mdx

## Diagnosis recipes

- **Agent not detected or wrong state:** Run `h0x agent list` to see what HyperOx sees and `h0x agent explain <target> --json` to see why the detector classified a pane that way. Integrations can provide lifecycle state, native session restore, or both; check `h0x integration status` and the agent support table before assuming an integration replaces screen detection. Details: https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/agents.mdx and https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/integrations.mdx
- **A keybinding does nothing:** the outer terminal or desktop environment owns that chord. Point the human to https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/keyboard.mdx to pick a safe one or free the chord in their terminal settings.
- **Something looks wrong at startup or with the socket API:** Default-session logs live in `~/.config/h0x/` on Linux and macOS and `%APPDATA%\h0x\` on Windows. Named-session logs live under `sessions/<name>/` inside that directory. `h0x status`, `h0x status server`, and `h0x status client` summarize the runtime.
- **Remote use:** SSH to the machine and run `h0x` there (works like tmux), or attach as a thin local client with `h0x --remote <host>`. Trade-offs: https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/how-to-work.mdx
- **What survives a detach, restart, or update:** https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/session-state.mdx

## Rules for you

- Do not invent keybindings, config keys, or CLI flags. The ones in this file are accurate as of writing; for anything else, read the linked docs page first.
- Teach mouse before keyboard for humans new to multiplexers.
- HyperOx is not tmux: do not give tmux commands, tmux config syntax, or `.tmux.conf` advice for HyperOx questions.
- For automation, scripting, or controlling HyperOx from code, point to the CLI reference (https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/cli-reference.mdx) and socket API (https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/socket-api.mdx).
