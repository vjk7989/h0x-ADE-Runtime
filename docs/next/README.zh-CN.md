# HyperOx


<p align="center">
  <img src="assets/logo.png" alt="HyperOx" width="100" />
</p>

<p align="center">
  <a href="https://github.com/vjk7989/h0x-ADE-Runtime">github.com/vjk7989/h0x-ADE-Runtime</a> · <a href="#安装">安装</a> · <a href="https://github.com/vjk7989/h0x-ADE-Runtime">快速开始</a> · <a href="https://github.com/vjk7989/h0x-ADE-Runtime">文档</a></p>

<p align="center">
  <a href="README.md">English</a> · 简体中文
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

**智能体复用器，住在你的终端里。**

- **每个智能体一目了然**——`blocked`、`working`、`done`。真实的终端视图，而不是包装过的转述。
- **分离后工作继续运行**——关闭客户端或 SSH 断线后，后台服务器仍会保持终端运行。服务器或机器重启后，HyperOx 会恢复已保存的布局，并可恢复受支持的智能体会话；原有进程不会保留。[会话状态 →](https://github.com/vjk7989/h0x-ADE-Runtime)
- **多台机器，一个窗口**——将本地工作和已保存的 SSH 机器放在一起，使用汇总的智能体列表，各连接独立重连。[远程机器 →](https://github.com/vjk7989/h0x-ADE-Runtime)
- **智能体也能使用 HyperOx**——纯 socket api：智能体可以创建窗格、读取输出、互相等待。[智能体技能 →](https://github.com/vjk7989/h0x-ADE-Runtime) 在开发智能体？[为你的智能体添加 HyperOx 支持 →](https://github.com/vjk7989/h0x-ADE-Runtime)
- **键盘和鼠标都是一等公民**——tmux 风格的前缀键，*以及*点击、拖动、分割。按当下的场景选择，而不是被工具锁死。
- **插件**——扩展窗格和工作流。[浏览插件市场 →](https://github.com/vjk7989/h0x-ADE-Runtime)
- **单个 rust 二进制，没有 electron**——运行在你已经在用的任何终端里。

---

## 安装

```bash
curl -fsSL https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest/download/install.sh | sh
```

或者 `brew install h0x` · `mise use -g h0x` · Windows：`powershell -ExecutionPolicy Bypass -c "irm https://github.com/vjk7989/h0x-ADE-Runtime/releases/latest/download/install.ps1 | iex"` · [受端点保护的 Windows](https://github.com/vjk7989/h0x-ADE-Runtime) · [二进制文件](https://github.com/vjk7989/h0x-ADE-Runtime/releases)

然后在工作所在的目录启动它：

```bash
h0x
```

运行你的智能体、分割窗格，然后安心离开。`ctrl+b q` 分离，`h0x` 重新连接。[快速开始 →](https://github.com/vjk7989/h0x-ADE-Runtime)

## 文档

所有文档都在 [github.com/vjk7989/h0x-ADE-Runtime/docs](https://github.com/vjk7989/h0x-ADE-Runtime)：[快速开始](https://github.com/vjk7989/h0x-ADE-Runtime) · [核心概念](https://github.com/vjk7989/h0x-ADE-Runtime) · [受支持的智能体](https://github.com/vjk7989/h0x-ADE-Runtime) · [键盘](https://github.com/vjk7989/h0x-ADE-Runtime) · [配置](https://github.com/vjk7989/h0x-ADE-Runtime) · [会话状态](https://github.com/vjk7989/h0x-ADE-Runtime) · [连接机器](https://github.com/vjk7989/h0x-ADE-Runtime) · [远程访问](https://github.com/vjk7989/h0x-ADE-Runtime) · [集成](https://github.com/vjk7989/h0x-ADE-Runtime) · [为智能体添加 HyperOx 支持](https://github.com/vjk7989/h0x-ADE-Runtime) · [插件](https://github.com/vjk7989/h0x-ADE-Runtime) · [socket api](https://github.com/vjk7989/h0x-ADE-Runtime)

## 致谢

<a href="https://terminaltrove.com/"><img src="assets/sponsors/terminal-trove.png" alt="Terminal Trove" width="200" /></a>

[Terminal Trove](https://terminaltrove.com/) 以及 [SPONSORS.md](./SPONSORS.md) 中列出的每一位支持者——谢谢 🐑

企业/合作：hey@github.com/vjk7989/h0x-ADE-Runtime

## 智能体须知

如果你是协助本仓库的 AI 智能体：在改动代码前阅读 [`AGENTS.md`](./AGENTS.md)，在创建 issue 或 PR 前阅读 [`CONTRIBUTING.md`](./CONTRIBUTING.md)。

## 开发

```bash
git clone https://github.com/vjk7989/h0x-ADE-Runtime
cd h0x
cargo build --release

just test        # 单元测试
just check       # 格式检查、测试和维护性检查
```

## 许可证

HyperOx 基于 [Apache License 2.0](LICENSE) 许可证发布。
