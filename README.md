# ttt — Terminal Typing Trainer

A TUI application for practicing typing with real source code. It displays syntax-highlighted code and lets you type along, tracking your speed and accuracy in real time.

## Features

- **Syntax Highlighting** — Powered by [syntect](https://github.com/trishume/syntect) with the GitHub Dark theme. Supports any language syntect recognizes (Rust, Python, JavaScript, C, Go, etc.).
- **Comment Skipping** — Automatically skips comment lines (line comments, block comments, doc comments) so you only type actual code. Inline comments (code + comment) only require typing the code portion.
- **Inline Overlay** — Type directly on top of the source code. Correct characters show in their original highlight color; mistakes appear with a red background.
- **Auto-indent** — Leading whitespace is filled in automatically. You only type the code itself.
- **Typo Enforcement** — If you make a mistake, you must fix it with Backspace before the line advances.
- **Enter to Advance** — After completing a line correctly, press Enter to move to the next line.
- **Ctrl+W Word Delete** — Delete one word backwards. Special characters (except `_`) are treated as individual words.
- **Line Selection Mode** — Use `--cursor` to browse the file and pick a starting line with j/k navigation.
- **Start Line** — Use `--line N` to jump directly to a specific line.
- **Stdin / Pipe Input** — Read source from stdin. Pipe any command output directly into ttt.
- **Diff Mode** — Use `--diff` to extract only added lines (`+`) from unified diff output. Extension is auto-detected from diff headers.
- **Source Filter** — Use `--src` with `--diff` to include only source code files (`.rs`, `.c`, `.py`, `.js`, etc.), skipping configs, docs, and other non-code files.
- **Extension Override** — Use `--ext` to specify the language for syntax highlighting, useful when piping or overriding file detection.
- **Per-Line KPM** — Completed lines show their individual KPM, color-coded relative to your average (green = above, yellow = near, red = below). Disable with `--quiet`. Use `--wpm` to show WPM (words per minute, 1 word = 5 keystrokes) instead.
- **Idle Time Excluded** — Timer pauses between lines, so only active typing time is measured.
- **Live Stats** — Real-time KPM (keystrokes per minute), accuracy percentage, line progress, and elapsed time.
- **Results Screen** — Final summary with KPM, accuracy, total time, and keystrokes when you finish the file.

## Installation

```bash
git clone https://github.com/your-username/ttt.git
cd ttt
cargo build --release
```

## Usage

```bash
# File input
cargo run -- <source-file>
cargo run -- --line 50 <source-file>       # start at line 50
cargo run -- --cursor <source-file>        # pick start line interactively
cargo run -- -l 50 -c <source-file>        # combine: cursor mode starting near line 50

# Stdin / pipe input
cat main.rs | cargo run -- -e rs           # pipe file, specify extension
echo "fn main() {}" | cargo run -- -e rs   # pipe any text
cargo run -- -e py -                       # explicit stdin with `-`

# Diff mode
git log -p | cargo run -- --diff           # type added lines from diff output
git diff HEAD~1 | cargo run -- -d          # short flag
git log -p | cargo run -- -d -s            # source code files only
git log -p -- src/ | cargo run -- -d       # limit to specific paths via git

# Extension override
cargo run -- --ext js <file>               # override file extension detection

# Quiet mode (hide per-line KPM)
cargo run -- --quiet <source-file>         # no per-line KPM display
cargo run -- -q <source-file>             # short flag

# WPM mode (show WPM instead of KPM)
cargo run -- --wpm <source-file>           # WPM in status bar + per-line
```

### Controls

#### Typing Mode

| Key | Action |
|-----|--------|
| Any character | Type the expected character |
| Enter | Advance to next line (complete and correct), or skip line (if not yet typed) |
| Backspace | Delete the last typed character on the current line |
| Ctrl+W | Delete one word backwards |
| Ctrl+R | Restart current line (reset typed chars and restore timer) |
| Ctrl+C | Quit |
| Ctrl+Q | Quit |

#### Line Selection Mode (`--cursor`)

| Key | Action |
|-----|--------|
| j / Down | Move cursor down |
| k / Up | Move cursor up |
| h / Left | Move cursor up |
| l / Right | Move cursor down |
| g / Home | Jump to first line |
| G / End | Jump to last line |
| Enter | Start typing from selected line |
| Ctrl+C | Quit |
| Ctrl+Q | Quit |

## Screenshot

```
   1  // comment line (dimmed, skipped)
   2  fn main() {
   3      let x = 42;█
   4      println!("{}", x);
   5  }
 KPM: 320  |  Acc: 97.5%  |  Line: 3/5  |  Time: 0:15  |  [Rust]
```

## License

This project is licensed under the [MIT License](LICENSE).
