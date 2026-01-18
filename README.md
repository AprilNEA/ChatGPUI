<!--
SPDX-FileCopyrightText: 2026 AprilNEA LLC
SPDX-License-Identifier: CC-BY-4.0
-->

<div align="center">

# ChatGPUI

A blazingly fast, GPU-accelerated native LLM chat client built with [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) — the same rendering engine that powers [Zed](https://zed.dev).

[![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/gpui-0.2-green.svg)](https://github.com/zed-industries/zed)

[Features](#features) • [Installation](#installation) • [Configuration](#configuration) • [Development](#development) • [License](#license)

</div>

---

## Features

- **GPU-Accelerated Rendering** - Leverages Metal/Vulkan for buttery-smooth 120fps UI with minimal CPU usage
- **Streaming Responses** - Real-time streaming output from LLM providers
- **Multi-Provider Support** - Connect to various LLM providers:
  - Anthropic (Claude)
  - OpenAI (GPT-4, GPT-4o)
  - Azure OpenAI
  - DeepSeek
  - Google AI (Gemini)
  - Groq
  - Mistral
  - Ollama (Local)
  - OpenRouter
- **Modern UI** - Clean, macOS-native interface with smooth animations
- **Collapsible Sidebar** - Chat history sidebar with animated toggle
- **Persistent Settings** - Configuration saved locally
- **i18n Support** - Internationalization ready


### Tech Stack

- **[GPUI](https://github.com/zed-industries/zed)** - GPU-accelerated UI framework
- **[gpui-component](https://github.com/longbridge/gpui-component)** - UI component library
- **[Tokio](https://tokio.rs/)** - Async runtime
- **[reqwest](https://docs.rs/reqwest)** - HTTP client
- **[serde](https://serde.rs/)** - Serialization

## License

This project is dual-licensed:

- **Open Source**: [AGPL-3.0](LICENSE) - Free for open-source projects
- **Commercial**: [Commercial License](LICENSES/LicenseRef-Commercial.txt) - For proprietary use

For commercial licensing inquiries, please contact [license@aprilnea.com](mailto:license@aprilnea.com).

---

<div align="center">

Made with ❤️ by [AprilNEA](https://github.com/anthropics)

</div>
