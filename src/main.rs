mod app;
mod parser;
mod stats;
mod ui;

use std::env;
use std::io::{IsTerminal, Read};
use std::path::Path;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

fn main() {
    if let Err(e) = run() {
        eprintln!("ttt: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let mut file_path: Option<String> = None;
    let mut start_line: usize = 1;
    let mut ext_override: Option<String> = None;
    let mut diff_mode = false;
    let mut source_only = false;
    let mut quiet = false;
    let mut wpm_mode = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--diff" | "-d" => {
                diff_mode = true;
            }
            "--src" | "-s" => {
                source_only = true;
            }
            "--quiet" | "-q" => {
                quiet = true;
            }
            "--wpm" => {
                wpm_mode = true;
            }
            "--line" | "-l" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --line requires a number");
                    std::process::exit(1);
                }
                start_line = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("Error: invalid line number '{}'", args[i]);
                    std::process::exit(1);
                });
            }
            "--ext" | "-e" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --ext requires an extension (e.g., rs, py, js)");
                    std::process::exit(1);
                }
                ext_override = Some(args[i].clone());
            }
            arg => {
                file_path = Some(arg.to_string());
            }
        }
        i += 1;
    }

    let use_stdin = file_path.as_deref() == Some("-")
        || (file_path.is_none() && !std::io::stdin().is_terminal());

    let mut content = if use_stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Some(buf)
    } else if let Some(ref path) = file_path {
        if diff_mode || ext_override.is_some() {
            Some(std::fs::read_to_string(path)?)
        } else {
            None
        }
    } else {
        eprintln!("Usage: ttt [OPTIONS] <file>");
        eprintln!("       command | ttt [OPTIONS]");
        eprintln!("\nRun 'ttt --help' for more information.");
        std::process::exit(1);
    };

    if diff_mode {
        let raw = content.as_deref().unwrap_or("");
        let (extracted, detected_ext) = extract_diff_additions(raw, source_only);
        if ext_override.is_none() {
            ext_override = detected_ext;
        }
        content = Some(extracted);
    }

    let (source_lines, syntax_name) = if let Some(ref text) = content {
        let ext = ext_override.as_deref().unwrap_or("txt");
        parser::parse_source(text, ext)?
    } else {
        parser::parse_file(file_path.as_deref().unwrap())?
    };

    if source_lines.is_empty() {
        eprintln!("Error: input is empty");
        std::process::exit(1);
    }

    let mut app = app::App::new(source_lines, syntax_name, start_line, quiet, wpm_mode);

    let mut terminal = ratatui::init();

    let result = run_loop(&mut terminal, &mut app);

    ratatui::restore();

    result
}

const SOURCE_EXTENSIONS: &[&str] = &[
    "c", "h", "cpp", "cc", "cxx", "hpp", "hxx", "cs", "rs", "go", "java", "kt", "kts", "scala",
    "py", "rb", "pl", "pm", "lua", "r", "js", "ts", "jsx", "tsx", "mjs", "cjs", "vue", "svelte",
    "swift", "m", "mm", "zig", "nim", "v", "d", "hs", "ml", "mli", "ex", "exs", "erl", "sh",
    "bash", "zsh", "fish", "ps1", "php", "dart", "jl", "sql", "proto", "thrift", "asm", "s",
    "wasm", "wat", "clj", "cljs", "lisp", "el", "scm", "rkt",
];

fn is_source_ext(ext: &str) -> bool {
    SOURCE_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

fn extract_diff_additions(input: &str, source_only: bool) -> (String, Option<String>) {
    let mut first_ext: Option<String> = None;
    let mut current_file_is_source = !source_only;
    let mut lines = Vec::new();

    for line in input.lines() {
        if line.starts_with("+++ b/") || line.starts_with("+++ a/") {
            let path = &line[6..];
            let ext = Path::new(path).extension().and_then(|e| e.to_str());
            current_file_is_source = match ext {
                Some(e) => !source_only || is_source_ext(e),
                None => !source_only,
            };
            if first_ext.is_none() && current_file_is_source {
                first_ext = ext.map(|s| s.to_string());
            }
        } else if current_file_is_source && line.starts_with('+') && !line.starts_with("+++") {
            lines.push(&line[1..]);
        }
    }

    (lines.join("\n"), first_ext)
}

fn print_help() {
    println!(
        "\
ttt - typing practice with syntax-highlighted source code

USAGE:
    ttt [OPTIONS] <file>
    ttt [OPTIONS] -
    command | ttt [OPTIONS]

ARGS:
    <file>                Source file to practice typing
    -                     Read from stdin explicitly

OPTIONS:
    -h, --help            Show this help message and exit
    -l, --line <N>        Start at line N (default: 1)
    -e, --ext <EXT>       Override syntax highlighting extension (e.g., rs, py, js)
    -d, --diff            Extract added lines from unified diff input
    -s, --src             With --diff, include only source code files
    -q, --quiet           Hide per-line speed display
        --wpm             Show speed as WPM instead of KPM

KEYBINDINGS:
    Enter                 Confirm current line and advance
    Backspace             Delete last typed character
    Ctrl+W                Delete last word
    Ctrl+R                Restart current line
    Ctrl+C, Ctrl+Q        Quit

EXAMPLES:
    ttt main.rs
    ttt --line 50 main.rs
    cat file.py | ttt -e py
    git diff HEAD~1 | ttt --diff
    git log -p | ttt --diff --src"
    );
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut app::App,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            if matches!(
                (key.modifiers, key.code),
                (KeyModifiers::CONTROL, KeyCode::Char('c' | 'q'))
            ) {
                break;
            }

            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('w')) => {
                    app.backspace_word();
                }
                (KeyModifiers::CONTROL, KeyCode::Char('r')) => {
                    app.restart_line();
                }
                (_, KeyCode::Enter) => app.confirm_line(),
                (_, KeyCode::Char(c)) => app.type_char(c),
                (_, KeyCode::Backspace) => app.backspace(),
                _ => {}
            }
        }
    }
    Ok(())
}
