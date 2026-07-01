# 🦀 Russee

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://img.shields.io/badge/Status-Development-blue.svg)](#)

**Russee** is an interactive terminal finder with two modes: a fuzzy file finder and a
grep-style content search. Type to filter, then press `Enter` to print the result to stdout or
`Ctrl+O` to open it in your editor. It walks the tree in parallel, respects `.gitignore`, and
ranks file matches with [`nucleo`](https://github.com/helix-editor/nucleo) (the matcher used by
Helix). A syntax-highlighted preview pane (via [`syntect`](https://github.com/trishume/syntect))
shows the selected file as you move.

---

## 🚀 Features

* **Fuzzy file finding:** `srcprs` finds `src/parser.rs`, and results re-rank as you type.
* **fzf-style extended syntax.** Refine a query with operators:

  | Token | Meaning |
  | :--- | :--- |
  | `foo` | fuzzy match |
  | `'foo` | exact / literal token |
  | `^foo` | anchored to the start |
  | `foo$` | anchored to the end |
  | `!foo` | negate (exclude) |
  | `a b` | AND (both required) |
  | `a \| b` | OR |

* **Content search.** Press `Tab` to grep file contents with the same extended syntax (literal
  by default; `Ctrl+R` toggles full regex). Matches show as `path:line: text`, and `Enter`
  prints `path:line`. Queries are debounced, and a new one cancels the previous search.
* **Smart-case.** A lowercase query is case-insensitive; any uppercase makes it case-sensitive.
  Toggle it with `Alt+C`.
* **Gitignore-aware and parallel.** Skips ignored files, hidden files, and binaries, using the
  same walker as ripgrep (`ignore`).
* **Live preview.** A syntax-highlighted pane shows the selected file, centered on the matched
  line in content mode. It reads only a window around the focus and caches it, so large files
  stay responsive. `Ctrl+T` toggles it; `Alt+↑`/`Alt+↓` scroll it.
* **Composable.** `Enter` prints the selected path to stdout, so it fits in a shell pipeline:
  `vim "$(rsc)"`, `cd "$(rsc)"`.

---

## ⌨️ Controls

| Action | Keybinding |
| :--- | :--- |
| **Filter** | Type characters directly |
| **Switch mode (Files ⇄ Content)** | `Tab` |
| **Toggle regex (content mode)** | `Ctrl+R` |
| **Toggle preview** | `Ctrl+T` |
| **Scroll preview** | `Alt+↑` / `Alt+↓` |
| **Navigate** | `↑`/`↓`, `Ctrl+P`/`Ctrl+N`, or `Ctrl+J`/`Ctrl+K` |
| **Page** | `PgUp` / `PgDn` |
| **Print selection & exit** | `Enter` |
| **Open in `$EDITOR`** | `Ctrl+O` |
| **Toggle case sensitivity** | `Alt+C` |
| **Edit query** | `←`/`→`, `Home`/`End` (`Ctrl+A`/`Ctrl+E`), `Backspace`, `Delete`, `Ctrl+W`, `Ctrl+U` |
| **Quit (no output)** | `Esc` / `Ctrl+C` / `Ctrl+Q` |

> `Ctrl+J`/`Ctrl+K` and `Alt+C` need a terminal that supports distinct key reporting. The
> arrow keys and `Ctrl+P`/`Ctrl+N` work everywhere.

---

## 🛠️ Usage

```bash
# Search the current directory
rsc

# Search a specific directory
rsc src

# Restrict to file types (repeatable)
rsc --type rust --type toml

# Include/exclude by glob (repeatable)
rsc -g '!*.lock' -g 'src/**'
```

Inside the TUI, press `Tab` to switch to content search and grep file contents.

`Enter` prints the chosen path (or `path:line` in content mode) and exits `0`. `Esc` exits `1`
with no output, so it works in scripts.

### Headless (no TUI)

```bash
# grep-style content search straight to stdout
rsc --cli checked_add
rsc --cli 'fn ' --type rust | head
rsc --cli Foo --ignore-case        # -i / --ignore-case, -s / --case-sensitive
```

---

## 📦 Installation

You need the [Rust toolchain](https://rustup.rs/) (edition 2024 / Rust 1.85+).

```bash
git clone https://github.com/yourusername/russee.git
cd russee
cargo install --path .     # builds and installs `rsc` onto your $PATH
```

Then run `rsc` from anywhere. To build locally without installing, run
`cargo build --release` and use `target/release/rsc`.

---

## ⚙️ Configuration

Russee reads `~/.config/russee/config.toml` (honoring `$XDG_CONFIG_HOME`) if present. A missing
file or a typo falls back to defaults. See [`config.example.toml`](config.example.toml).

```toml
editor = "nvim"                          # Ctrl+O opens this at the line
# editor_cmd = "code -g {file}:{line}"   # template override; wins over `editor`
theme = "Dracula"                        # preview theme
```

**Editor.** Known names get the correct line flag automatically. Precedence is
`editor_cmd`, then `editor`, then `$EDITOR`, then `vi`:

| Editors | Invocation |
| :--- | :--- |
| `vi` `vim` `nvim` `nano` `emacs` `micro` | `<ed> +LINE FILE` |
| `code` `code-insiders` `codium` | `code -g FILE:LINE` |
| `zed` `subl` `hx` | `<ed> FILE:LINE` |

For any other editor, set `editor_cmd` with `{file}`/`{line}` placeholders.

**Theme.** One of the bundled themes (`Nord`, `Dracula`, `gruvbox-dark`, `Solarized (dark)`,
`OneHalfDark`, `Monokai Extended`, and more), `ansi`/`base16` to follow your terminal's colors,
or the stem of a custom `.tmTheme` in `~/.config/russee/themes/`.

---

## 📄 License

MIT
