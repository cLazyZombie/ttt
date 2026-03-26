use ratatui::style::{Color, Modifier, Style};
use std::io::Cursor;
use std::path::Path;
use syntect::highlighting::{
    FontStyle, HighlightIterator, HighlightState, Highlighter, Theme, ThemeSet,
};
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxSet};

const GITHUB_DARK_THEME: &[u8] = include_bytes!("../themes/github-dark.tmTheme");

fn load_github_dark_theme() -> Theme {
    let mut reader = Cursor::new(GITHUB_DARK_THEME);
    ThemeSet::load_from_reader(&mut reader).expect("Failed to load embedded GitHub Dark theme")
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineType {
    Code,
    Comment,
    Empty,
    Mixed,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub style: Style,
    pub is_comment: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SourceLine {
    pub raw: String,
    pub tokens: Vec<Token>,
    pub typeable_content: String,
    pub typeable_styles: Vec<Style>,
    pub leading_whitespace: String,
    pub line_type: LineType,
    pub comment_suffix: Option<String>,
}

fn convert_style(syntect_style: &syntect::highlighting::Style) -> Style {
    let fg = Color::Rgb(
        syntect_style.foreground.r,
        syntect_style.foreground.g,
        syntect_style.foreground.b,
    );

    let mut modifier = Modifier::empty();
    if syntect_style.font_style.contains(FontStyle::BOLD) {
        modifier |= Modifier::BOLD;
    }
    if syntect_style.font_style.contains(FontStyle::ITALIC) {
        modifier |= Modifier::ITALIC;
    }
    if syntect_style.font_style.contains(FontStyle::UNDERLINE) {
        modifier |= Modifier::UNDERLINED;
    }

    Style::default().fg(fg).add_modifier(modifier)
}

fn is_comment_scope(stack: &ScopeStack, comment_scope: Scope) -> bool {
    stack
        .as_slice()
        .iter()
        .any(|s| comment_scope.is_prefix_of(*s))
}

fn build_source_line(raw: &str, tokens: Vec<Token>) -> SourceLine {
    let trimmed = raw.trim_start();
    let leading_ws_len = raw.len() - trimmed.len();
    let leading_whitespace = raw[..leading_ws_len].to_string();

    if trimmed.is_empty() {
        return SourceLine {
            raw: raw.to_string(),
            tokens,
            typeable_content: String::new(),
            typeable_styles: Vec::new(),
            leading_whitespace,
            line_type: LineType::Empty,
            comment_suffix: None,
        };
    }

    let has_code = tokens
        .iter()
        .any(|t| !t.is_comment && !t.text.trim().is_empty());
    let has_comment = tokens
        .iter()
        .any(|t| t.is_comment && !t.text.trim().is_empty());

    let line_type = match (has_code, has_comment) {
        (true, true) => LineType::Mixed,
        (true, false) => LineType::Code,
        (false, true) => LineType::Comment,
        (false, false) => LineType::Empty,
    };

    match line_type {
        LineType::Comment | LineType::Empty => SourceLine {
            raw: raw.to_string(),
            tokens,
            typeable_content: String::new(),
            typeable_styles: Vec::new(),
            leading_whitespace,
            line_type,
            comment_suffix: None,
        },
        LineType::Code | LineType::Mixed => {
            let mut code_chars: Vec<char> = Vec::new();
            let mut code_styles: Vec<Style> = Vec::new();
            let mut comment_part = String::new();
            let mut in_comment_region = false;
            let mut byte_pos = 0;

            for token in &tokens {
                let token_end = byte_pos + token.text.len();

                if token.is_comment && byte_pos >= leading_ws_len {
                    in_comment_region = true;
                }

                if in_comment_region {
                    comment_part.push_str(&token.text);
                } else if !token.is_comment {
                    let mut local_byte = byte_pos;
                    for ch in token.text.chars() {
                        if local_byte >= leading_ws_len {
                            code_chars.push(ch);
                            code_styles.push(token.style);
                        }
                        local_byte += ch.len_utf8();
                    }
                }

                byte_pos = token_end;
            }

            // Trim trailing whitespace from code part
            while code_chars.last().is_some_and(|c| c.is_whitespace()) {
                code_chars.pop();
                code_styles.pop();
            }

            let typeable_content: String = code_chars.into_iter().collect();
            let comment_suffix = if comment_part.is_empty() {
                None
            } else {
                Some(comment_part)
            };

            SourceLine {
                raw: raw.to_string(),
                tokens,
                typeable_content,
                typeable_styles: code_styles,
                leading_whitespace,
                line_type,
                comment_suffix,
            }
        }
    }
}

pub fn parse_file(path: &str) -> Result<(Vec<SourceLine>, String), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let extension = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("txt");
    parse_source(&content, extension)
}

pub fn parse_source(
    content: &str,
    extension: &str,
) -> Result<(Vec<SourceLine>, String), Box<dyn std::error::Error>> {
    let content = content.replace('\t', "    ");
    let lines: Vec<&str> = content.lines().collect();

    let ss = SyntaxSet::load_defaults_newlines();
    let theme = load_github_dark_theme();

    let syntax = ss
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let syntax_name = syntax.name.clone();

    let highlighter = Highlighter::new(&theme);
    let mut parse_state = ParseState::new(syntax);
    let mut highlight_state = HighlightState::new(&highlighter, ScopeStack::new());

    let comment_scope =
        Scope::new("comment").map_err(|e| format!("Failed to create comment scope: {e:?}"))?;

    let mut source_lines = Vec::new();
    let mut scope_stack = ScopeStack::new(); // persist across lines for block comments

    for line in &lines {
        let line_nl = format!("{line}\n");
        let ops = parse_state.parse_line(&line_nl, &ss)?;

        // Get styled regions from HighlightIterator
        let regions: Vec<(syntect::highlighting::Style, String)> =
            HighlightIterator::new(&mut highlight_state, &ops[..], &line_nl, &highlighter)
                .map(|(s, t)| (s, t.to_string()))
                .collect();

        // Build comment map using scope tracking (scope_stack persists across lines)
        let mut comment_map = vec![false; line_nl.len()];

        let mut last_pos = 0;
        for &(pos, ref op) in &ops {
            let is_comment = is_comment_scope(&scope_stack, comment_scope);
            for item in comment_map.iter_mut().take(pos).skip(last_pos) {
                *item = is_comment;
            }
            scope_stack.apply(op)?;
            last_pos = pos;
        }
        // Remaining characters
        let is_comment = is_comment_scope(&scope_stack, comment_scope);
        for item in comment_map.iter_mut().take(line_nl.len()).skip(last_pos) {
            *item = is_comment;
        }

        // Merge regions with comment info
        let mut pos = 0;
        let mut tokens = Vec::new();
        for (syntect_style, text) in &regions {
            let token_is_comment = if pos < comment_map.len() {
                comment_map[pos]
            } else {
                false
            };

            // Skip the trailing newline token
            let text_clean = if pos + text.len() >= line_nl.len() {
                text.trim_end_matches('\n').to_string()
            } else {
                text.clone()
            };

            if !text_clean.is_empty() {
                tokens.push(Token {
                    text: text_clean,
                    style: convert_style(syntect_style),
                    is_comment: token_is_comment,
                });
            }

            pos += text.len();
        }

        let source_line = build_source_line(line, tokens);
        source_lines.push(source_line);
    }

    Ok((source_lines, syntax_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_rust(code: &str) -> Vec<SourceLine> {
        let (lines, syntax) = parse_source(code, "rs").unwrap();
        assert_eq!(syntax, "Rust");
        lines
    }

    #[test]
    fn github_dark_theme_loads() {
        let theme = load_github_dark_theme();
        // Verify theme has basic settings (foreground/background)
        let bg = theme.settings.background.unwrap();
        let fg = theme.settings.foreground.unwrap();
        // GitHub Dark should have a dark background
        assert!(bg.r < 100, "Expected dark background, got r={}", bg.r);
        // And a light foreground
        assert!(fg.r > 100, "Expected light foreground, got r={}", fg.r);
    }

    #[test]
    fn github_dark_theme_highlights_code() {
        // Verify the theme actually produces styled output
        let lines = parse_rust("let x = 42;");
        let line = &lines[0];
        // Should have styles with non-default RGB colors from GitHub Dark theme
        assert!(!line.typeable_styles.is_empty());
        let has_colored_style = line
            .typeable_styles
            .iter()
            .any(|s| matches!(s.fg, Some(ratatui::style::Color::Rgb(_, _, _))));
        assert!(has_colored_style, "Theme should produce RGB-colored styles");
    }

    #[test]
    fn empty_source() {
        let (lines, _) = parse_source("", "rs").unwrap();
        assert!(lines.is_empty() || lines.iter().all(|l| l.line_type == LineType::Empty));
    }

    #[test]
    fn pure_code_line() {
        let lines = parse_rust("let x = 42;");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_type, LineType::Code);
        assert_eq!(lines[0].typeable_content, "let x = 42;");
        assert!(lines[0].leading_whitespace.is_empty());
        assert!(lines[0].comment_suffix.is_none());
    }

    #[test]
    fn code_with_leading_whitespace() {
        let lines = parse_rust("    let x = 42;");
        assert_eq!(lines[0].line_type, LineType::Code);
        assert_eq!(lines[0].leading_whitespace, "    ");
        assert_eq!(lines[0].typeable_content, "let x = 42;");
    }

    #[test]
    fn comment_line_detected() {
        let lines = parse_rust("// this is a comment");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_type, LineType::Comment);
        assert!(lines[0].typeable_content.is_empty());
    }

    #[test]
    fn indented_comment_line() {
        let lines = parse_rust("    // indented comment");
        assert_eq!(lines[0].line_type, LineType::Comment);
        assert!(lines[0].typeable_content.is_empty());
    }

    #[test]
    fn empty_line_detected() {
        let lines = parse_rust("\n");
        // "\n".lines() yields one empty string
        assert!(!lines.is_empty());
        assert_eq!(lines[0].line_type, LineType::Empty);
    }

    #[test]
    fn whitespace_only_line_is_empty() {
        let lines = parse_rust("    ");
        assert_eq!(lines[0].line_type, LineType::Empty);
        assert!(lines[0].typeable_content.is_empty());
    }

    #[test]
    fn mixed_line_code_and_comment() {
        let lines = parse_rust("let x = 5; // set x");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_type, LineType::Mixed);
        assert_eq!(lines[0].typeable_content, "let x = 5;");
        assert!(lines[0].comment_suffix.is_some());
        let suffix = lines[0].comment_suffix.as_ref().unwrap();
        assert!(suffix.contains("// set x"));
    }

    #[test]
    fn block_comment_single_line() {
        let lines = parse_rust("/* block comment */");
        assert_eq!(lines[0].line_type, LineType::Comment);
        assert!(lines[0].typeable_content.is_empty());
    }

    #[test]
    fn block_comment_multiline() {
        let code = "/* start\nmiddle\nend */";
        let lines = parse_rust(code);
        assert_eq!(lines.len(), 3);
        for line in &lines {
            assert_eq!(line.line_type, LineType::Comment);
            assert!(line.typeable_content.is_empty());
        }
    }

    #[test]
    fn multiline_source_mixed_types() {
        let code = "// comment\nfn main() {\n    // inner\n    let x = 1;\n}";
        let lines = parse_rust(code);
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0].line_type, LineType::Comment);
        assert_eq!(lines[1].line_type, LineType::Code);
        assert_eq!(lines[2].line_type, LineType::Comment);
        assert_eq!(lines[3].line_type, LineType::Code);
        assert_eq!(lines[4].line_type, LineType::Code);
    }

    #[test]
    fn tab_expansion() {
        let lines = parse_rust("\tlet x = 1;");
        assert_eq!(lines[0].leading_whitespace, "    ");
    }

    #[test]
    fn typeable_styles_length_matches_content() {
        let lines = parse_rust("let x = 42;");
        let line = &lines[0];
        assert_eq!(
            line.typeable_styles.len(),
            line.typeable_content.chars().count()
        );
    }

    #[test]
    fn syntax_detection_via_extension() {
        let (_, syntax) = parse_source("fn main() {}", "rs").unwrap();
        assert_eq!(syntax, "Rust");
    }

    #[test]
    fn unknown_extension_falls_back() {
        let result = parse_source("hello", "zzz_unknown");
        assert!(result.is_ok());
    }

    #[test]
    fn nonexistent_file_returns_error() {
        let result = parse_file("/nonexistent/path/file.rs");
        assert!(result.is_err());
    }

    #[test]
    fn comments_in_function_body() {
        let code = "fn main() {\n    // line comment\n    /* block comment */\n    let x = 1;\n}";
        let lines = parse_rust(code);
        assert_eq!(lines[0].line_type, LineType::Code, "fn main() {{");
        assert_eq!(lines[1].line_type, LineType::Comment, "    // line comment");
        assert_eq!(
            lines[2].line_type,
            LineType::Comment,
            "    /* block comment */"
        );
        assert_eq!(lines[3].line_type, LineType::Code, "    let x = 1;");
        assert_eq!(lines[4].line_type, LineType::Code, "}}");
        // Verify comment lines have no typeable content
        assert!(lines[1].typeable_content.is_empty());
        assert!(lines[2].typeable_content.is_empty());
    }

    #[test]
    fn multiline_block_comment_in_function() {
        let code = "fn f() {\n    /*\n     * multi\n     */\n    let x = 1;\n}";
        let lines = parse_rust(code);
        assert_eq!(lines[0].line_type, LineType::Code, "fn f() {{");
        assert_eq!(lines[1].line_type, LineType::Comment, "    /*");
        assert_eq!(lines[2].line_type, LineType::Comment, "     * multi");
        assert_eq!(lines[3].line_type, LineType::Comment, "     */");
        assert_eq!(lines[4].line_type, LineType::Code, "    let x = 1;");
        assert_eq!(lines[5].line_type, LineType::Code, "}}");
    }

    #[test]
    fn spaced_comments_various() {
        // Test with various indentation levels
        let code = "fn f() {\n  // 2-space indent\n      // 6-space indent\n  let x = 1;\n}";
        let lines = parse_rust(code);
        assert_eq!(lines[1].line_type, LineType::Comment, "  // 2-space");
        assert_eq!(lines[2].line_type, LineType::Comment, "      // 6-space");
        assert!(lines[1].typeable_content.is_empty());
        assert!(lines[2].typeable_content.is_empty());
    }

    #[test]
    fn python_hash_comments() {
        let code = "# comment\n    # indented\nx = 1  # inline\ndef f():\n    pass";
        let (lines, _) = parse_source(code, "py").unwrap();
        assert_eq!(lines[0].line_type, LineType::Comment, "# comment");
        assert_eq!(lines[1].line_type, LineType::Comment, "    # indented");
        assert_eq!(lines[2].line_type, LineType::Mixed, "x = 1  # inline");
        assert_eq!(lines[2].typeable_content, "x = 1");
        assert_eq!(lines[3].line_type, LineType::Code, "def f():");
        assert_eq!(lines[4].line_type, LineType::Code, "    pass");
    }

    #[test]
    fn c_style_comments() {
        let code = "// line comment\n    // indented\n/* block */\nint x = 1; /* inline */";
        let (lines, _) = parse_source(code, "c").unwrap();
        assert_eq!(lines[0].line_type, LineType::Comment, "// line");
        assert_eq!(lines[1].line_type, LineType::Comment, "    // indented");
        assert_eq!(lines[2].line_type, LineType::Comment, "/* block */");
        assert_eq!(
            lines[3].line_type,
            LineType::Mixed,
            "int x = 1; /* inline */"
        );
        assert_eq!(lines[3].typeable_content, "int x = 1;");
    }

    #[test]
    fn comments_inside_impl_block() {
        // Reproduces the pattern: code, empty, indented //, code
        let code = "\
impl App {
    pub fn new() -> Self {
        let x = 1;

        // this is a comment
        let y = 2;
    }
}";
        let lines = parse_rust(code);
        assert_eq!(lines[0].line_type, LineType::Code, "impl App {{");
        assert_eq!(lines[1].line_type, LineType::Code, "pub fn new()");
        assert_eq!(lines[2].line_type, LineType::Code, "let x = 1;");
        assert_eq!(lines[3].line_type, LineType::Empty, "empty line");
        assert_eq!(
            lines[4].line_type,
            LineType::Comment,
            "        // this is a comment"
        );
        assert!(lines[4].typeable_content.is_empty());
        assert_eq!(lines[5].line_type, LineType::Code, "let y = 2;");
    }

    #[test]
    fn deeply_indented_comments() {
        let code = "\
fn f() {
    if true {
        for i in 0..10 {
            // deeply indented comment
            /* block inside nested */
            let x = i;
        }
    }
}";
        let lines = parse_rust(code);
        assert_eq!(lines[3].line_type, LineType::Comment, "deeply indented //");
        assert_eq!(
            lines[4].line_type,
            LineType::Comment,
            "deeply indented /* */"
        );
        assert!(lines[3].typeable_content.is_empty());
        assert!(lines[4].typeable_content.is_empty());
    }

    #[test]
    fn comment_between_code_lines() {
        let code = "\
let a = 1;
// middle comment
let b = 2;";
        let lines = parse_rust(code);
        assert_eq!(lines[0].line_type, LineType::Code);
        assert_eq!(lines[1].line_type, LineType::Comment);
        assert!(lines[1].typeable_content.is_empty());
        assert_eq!(lines[2].line_type, LineType::Code);
    }

    #[test]
    fn empty_then_comment_then_code() {
        let code = "\
let x = 1;

    // comment after blank
    let y = 2;";
        let lines = parse_rust(code);
        assert_eq!(lines[0].line_type, LineType::Code);
        assert_eq!(lines[1].line_type, LineType::Empty);
        assert_eq!(lines[2].line_type, LineType::Comment);
        assert!(lines[2].typeable_content.is_empty());
        assert_eq!(lines[3].line_type, LineType::Code);
    }

    #[test]
    fn multiline_block_in_impl() {
        let code = "\
impl Foo {
    /*
     * Block comment
     * spanning lines
     */
    fn bar() {}
}";
        let lines = parse_rust(code);
        assert_eq!(lines[0].line_type, LineType::Code, "impl Foo");
        assert_eq!(lines[1].line_type, LineType::Comment, "/*");
        assert_eq!(lines[2].line_type, LineType::Comment, "* Block comment");
        assert_eq!(lines[3].line_type, LineType::Comment, "* spanning lines");
        assert_eq!(lines[4].line_type, LineType::Comment, "*/");
        assert_eq!(lines[5].line_type, LineType::Code, "fn bar()");
        for i in 1..=4 {
            assert!(
                lines[i].typeable_content.is_empty(),
                "line {} should have no typeable content",
                i + 1
            );
        }
    }

    #[test]
    fn debug_comment_detection() {
        let cases: Vec<(&str, LineType)> = vec![
            ("// comment", LineType::Comment),
            ("  // spaced", LineType::Comment),
            ("    // indented", LineType::Comment),
            ("/* block */", LineType::Comment),
            ("  /* spaced block */", LineType::Comment),
            ("    /* indented block */", LineType::Comment),
            ("/// doc comment", LineType::Comment),
            ("//! inner doc", LineType::Comment),
        ];

        for (code, expected_type) in &cases {
            let lines = parse_rust(code);
            assert_eq!(
                lines[0].line_type,
                *expected_type,
                "Failed for {:?}: got {:?}, tokens: {:?}",
                code,
                lines[0].line_type,
                lines[0]
                    .tokens
                    .iter()
                    .map(|t| (&t.text, t.is_comment))
                    .collect::<Vec<_>>()
            );
            assert!(
                lines[0].typeable_content.is_empty(),
                "Expected empty typeable for {:?}, got {:?}",
                code,
                lines[0].typeable_content
            );
        }
    }

    #[test]
    fn parse_file_success() {
        let (lines, syntax) = parse_file("test_sample.rs").unwrap();
        assert_eq!(syntax, "Rust");
        assert_eq!(lines.len(), 13);
        // Line 1: "// This is a comment line..."
        assert_eq!(lines[0].line_type, LineType::Comment, "line 1");
        // Line 2: "fn main() {"
        assert_eq!(lines[1].line_type, LineType::Code, "line 2");
        // Line 3: "    // Another comment..."
        assert_eq!(lines[2].line_type, LineType::Comment, "line 3: indented //");
        // Line 4: "    let x = 42;"
        assert_eq!(lines[3].line_type, LineType::Code, "line 4");
        // Line 5: "    let y = x + 1; // inline comment"
        assert_eq!(
            lines[4].line_type,
            LineType::Mixed,
            "line 5: inline comment"
        );
        assert_eq!(lines[4].typeable_content, "let y = x + 1;");
        // Line 6: "    println!..."
        assert_eq!(lines[5].line_type, LineType::Code, "line 6");
        // Line 7: empty
        assert_eq!(lines[6].line_type, LineType::Empty, "line 7: empty");
        // Line 8: "    /* Block comment"
        assert_eq!(lines[7].line_type, LineType::Comment, "line 8: /* start");
        // Line 9: "       spanning..."
        assert_eq!(
            lines[8].line_type,
            LineType::Comment,
            "line 9: block middle"
        );
        // Line 10: "       should all be skipped */"
        assert_eq!(
            lines[9].line_type,
            LineType::Comment,
            "line 10: block end */"
        );
        // Line 11: "    let result = x * y;"
        assert_eq!(lines[10].line_type, LineType::Code, "line 11");
        // Line 12: "    println!..."
        assert_eq!(lines[11].line_type, LineType::Code, "line 12");
        // Line 13: "}"
        assert_eq!(lines[12].line_type, LineType::Code, "line 13");
    }

    #[test]
    fn convert_style_bold() {
        let s = syntect::highlighting::Style {
            foreground: syntect::highlighting::Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            font_style: FontStyle::BOLD,
        };
        let result = convert_style(&s);
        assert_eq!(result.fg, Some(Color::Rgb(255, 0, 0)));
        assert!(result.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn convert_style_italic() {
        let s = syntect::highlighting::Style {
            foreground: syntect::highlighting::Color {
                r: 0,
                g: 128,
                b: 0,
                a: 255,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            font_style: FontStyle::ITALIC,
        };
        let result = convert_style(&s);
        assert!(result.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn convert_style_underline() {
        let s = syntect::highlighting::Style {
            foreground: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 255,
                a: 255,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            font_style: FontStyle::UNDERLINE,
        };
        let result = convert_style(&s);
        assert!(result.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn convert_style_all_modifiers() {
        let s = syntect::highlighting::Style {
            foreground: syntect::highlighting::Color {
                r: 1,
                g: 2,
                b: 3,
                a: 255,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            font_style: FontStyle::BOLD | FontStyle::ITALIC | FontStyle::UNDERLINE,
        };
        let result = convert_style(&s);
        assert!(result.add_modifier.contains(Modifier::BOLD));
        assert!(result.add_modifier.contains(Modifier::ITALIC));
        assert!(result.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn convert_style_no_modifiers() {
        let s = syntect::highlighting::Style {
            foreground: syntect::highlighting::Color {
                r: 10,
                g: 20,
                b: 30,
                a: 255,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            font_style: FontStyle::empty(),
        };
        let result = convert_style(&s);
        assert_eq!(result.fg, Some(Color::Rgb(10, 20, 30)));
        assert_eq!(result.add_modifier, Modifier::empty());
    }

    #[test]
    fn code_after_block_comment() {
        // Covers: comment_map falling back to false after block comment ends
        let code = "/* comment */\nlet x = 1;";
        let lines = parse_rust(code);
        assert_eq!(lines[0].line_type, LineType::Comment);
        assert_eq!(lines[1].line_type, LineType::Code);
        assert_eq!(lines[1].typeable_content, "let x = 1;");
    }

    #[test]
    fn mixed_line_with_leading_whitespace() {
        let lines = parse_rust("    let x = 5; // init");
        assert_eq!(lines[0].line_type, LineType::Mixed);
        assert_eq!(lines[0].leading_whitespace, "    ");
        assert_eq!(lines[0].typeable_content, "let x = 5;");
        assert!(lines[0].comment_suffix.is_some());
    }
}
