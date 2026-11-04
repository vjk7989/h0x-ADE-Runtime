# HyperOx


<p align="center">
  <img src="assets/logo.png" alt="HyperOx" width="100" />
</p>

<p align="center">
  <a href="https://github.com/vjk7989/h0x-ADE-Runtime">github.com/vjk7989/h0x-ADE-Runtime</a> · <a href="#install">install</a> · <a href="https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/quick-start.mdx">quick start</a> · <a href="https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/index.mdx">docs</a>
</p>

<p align="center">
  English · <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-666666?labelColor=333333" alt="Apache 2.0 license" /></a>
  <a href="https://github.com/vjk7989/h0x-ADE-Runtime/releases"><img src="https://img.shields.io/github/downloads/vjk7989/h0x-ADE-Runtime/total?labelColor=333333&color=666666" alt="total GitHub release downloads" /></a>
  <a href="https://github.com/vjk7989/h0x-ADE-Runtime/stargazers"><img src="https://img.shields.io/github/stars/vjk7989/h0x-ADE-Runtime?labelColor=333333&color=666666&logo=github" alt="GitHub stars" /></a>
  <a href="https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest"><img src="https://img.shields.io/github/v/release/vjk7989/h0x-ADE-Runtime?label=release&labelColor=333333&color=666666" alt="latest stable release" /></a>
  <a href="https://formulae.brew.sh/formula/h0x"><img src="https://img.shields.io/homebrew/v/h0x?label=homebrew&labelColor=333333&color=666666" alt="Homebrew version" /></a>
  <a href="https://github.com/vjk7989"><img src="https://img.shields.io/badge/follow-%40vjk7989-000000?logo=x&logoColor=white" alt="follow @vjk7989 on X" /></a>
</p>

---

https://github.com/user-attachments/assets/043ec09f-4bdd-41d5-aee0-8fda6b83e267

**the runtime your coding agents live on.**

- **detach without stopping work** — HyperOx keeps terminals running in a background server when you close the client or lose your SSH connection. after a server or machine restart, HyperOx restores the saved layout and can resume supported agent sessions; the original processes do not survive. [session state →](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/session-state.mdx)
- **several machines, one window** — keep local work and saved ssh machines together, with a combined agent list and independent reconnects. [remote machines →](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/connecting-machines.mdx)
- **never hunt for the stuck one** — every pane is marked working, blocked, or idle. when an agent stops and needs an answer, HyperOx says so.
- **agent-native** — agents drive HyperOx through the cli and socket api: they can spawn panes, prompt each other, and wait until another agent is genuinely blocked. [agent skill →](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/agent-skill.mdx)
- **runs what you already run** — claude code, codex, cursor, opencode, grok and the rest. HyperOx doesn't wrap or replace them; it owns their terminals. building an agent? [add HyperOx support →](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/add-h0x-support.mdx)
- **keyboard and mouse, both first-class** — tmux-style prefix keys *and* click, drag, split. pick per moment, not per tool.
- **plugins** — extend panes and workflows. [browse the marketplace →](https://github.com/vjk7989/h0x-ADE-Runtime)
- **one rust binary, no electron** — runs in whatever terminal you already use.

---

## install

```bash
curl -fsSL https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest/download/install.sh | sh
```

or `brew install h0x` · `mise use -g h0x` · windows: `powershell -ExecutionPolicy Bypass -c "irm https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest/download/install.ps1 | iex"` · [endpoint-protected Windows](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/windows-beta.mdx) · [binaries](https://github.com/vjk7989/h0x-ADE-Runtime/releases)

then start it where the work lives:

```bash
h0x
```

run your agents, split panes, walk away. `ctrl+b q` detaches, `h0x` reattaches. [quick start →](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/quick-start.mdx)

## docs

everything lives at [github.com/vjk7989/h0x-ADE-Runtime/docs](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/index.mdx): [quick start](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/quick-start.mdx) · [concepts](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/concepts.mdx) · [supported agents](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/agents.mdx) · [keyboard](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/keyboard.mdx) · [configuration](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/configuration.mdx) · [session state](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/session-state.mdx) · [connecting machines](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/connecting-machines.mdx) · [remote](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/persistence-remote.mdx) · [integrations](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/integrations.mdx) · [add HyperOx support to your agent](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/add-h0x-support.mdx) · [plugins](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/plugins.mdx) · [socket api](https://github.com/vjk7989/h0x-ADE-Runtime/blob/main/docs/next/website/src/content/docs/socket-api.mdx)

## thanks

every past sponsor and backer is listed in [SPONSORS.md](./SPONSORS.md) — thank you 🐑

enterprise / partnership: hey@github.com/vjk7989/h0x-ADE-Runtime

## agent instructions

if you are an ai agent helping with this repository, read [`AGENTS.md`](./AGENTS.md) before making changes and read [`CONTRIBUTING.md`](./CONTRIBUTING.md) before opening issues or PRs.

## development

```bash
git clone https://github.com/vjk7989/h0x-ADE-Runtime
cd h0x
cargo build --release

just test        # unit tests
just check       # formatting, tests, and maintenance checks
```

## license

HyperOx is licensed under the [Apache License 2.0](LICENSE).
