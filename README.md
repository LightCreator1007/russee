# 🦀 Russee

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://img.shields.io/badge/Status-Development-blue.svg)](#)

**Russee** is a high-performance, asynchronous terminal file searcher. By offloading heavy filesystem traversal to background worker threads, it ensures a fluid, lag-free user experience even when searching through massive codebases.

---

<div align="center">

### 🖥️ Interface Preview

![Russee Interface](https://github.com/user-attachments/assets/f4dafe36-892b-47e6-a16f-f5f156d9e96f)

*Asynchronous search results streamed directly to a responsive TUI.*

</div>

---

## 🚀 Key Capabilities

* **Asynchronous Architecture:** Utilizes `mpsc` channels to stream search results from background threads, keeping the UI thread unblocked and responsive.
* **Regex Engine:** Leverages the industry-standard `regex` crate for powerful, pattern-based discovery.
* **Real-time Updates:** Match counts and result lists update dynamically as the search progresses.
* **Native TUI:** Built with `ratatui` and `crossterm` for a low-latency, cross-platform terminal experience.

---

## ⌨️ Controls

| Action | Keybinding |
| :--- | :--- |
| **Search** | Type characters directly |
| **Navigate** | `Up` / `Down` Arrows |
| **Delete** | `Backspace` |
| **Quit** | `Ctrl + Q` |

---

## 🛠️ Installation

Ensure you have the [Rust toolchain](https://rustup.rs/) installed.

```bash
# Clone the repository
git clone [https://github.com/yourusername/russee.git](https://github.com/yourusername/russee.git)

# Navigate to the directory
cd russee

# Build and run
cargo run --release
