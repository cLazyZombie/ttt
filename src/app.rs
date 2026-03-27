use std::time::{Duration, Instant};

use crate::parser::{LineType, SourceLine};
use crate::stats::{LineStats, Stats};

/// Returns true if the character is part of a word (alphanumeric or `_`).
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub struct App {
    pub source_lines: Vec<SourceLine>,
    pub current_line: usize,
    pub current_col: usize,
    pub typed_chars: Vec<Vec<Option<char>>>,
    pub stats: Stats,
    pub scroll_offset: usize,
    pub finished: bool,
    pub syntax_name: String,
    pub line_stats: Vec<Option<LineStats>>,
    line_start: Option<Instant>,
    line_keystrokes: usize,
    line_correct: usize,
    stats_at_line_start: (usize, usize, Duration),
    pub quiet: bool,
    pub wpm_mode: bool,
}

impl App {
    pub fn new(
        source_lines: Vec<SourceLine>,
        syntax_name: String,
        start_line: usize,
        quiet: bool,
        wpm_mode: bool,
    ) -> Self {
        let typed_chars: Vec<Vec<Option<char>>> = source_lines
            .iter()
            .map(|sl| vec![None; sl.typeable_content.chars().count()])
            .collect();

        // start_line is 1-based from user; convert to 0-based
        let start_idx = start_line.saturating_sub(1);

        let line_count = source_lines.len();
        let mut app = App {
            source_lines,
            current_line: start_idx,
            current_col: 0,
            typed_chars,
            stats: Stats::new(),
            scroll_offset: 0,
            finished: false,
            syntax_name,
            line_stats: vec![None; line_count],
            line_start: None,
            line_keystrokes: 0,
            line_correct: 0,
            stats_at_line_start: (0, 0, Duration::ZERO),
            quiet,
            wpm_mode,
        };

        if start_idx >= app.source_lines.len() {
            app.finished = true;
        } else if !app.is_typeable_line(app.current_line) {
            app.advance_to_next_typeable_line();
            if app.current_line >= app.source_lines.len() {
                app.finished = true;
            }
        }

        app
    }

    fn is_typeable_line(&self, line_idx: usize) -> bool {
        if line_idx >= self.source_lines.len() {
            return false;
        }
        matches!(
            self.source_lines[line_idx].line_type,
            LineType::Code | LineType::Mixed
        )
    }

    pub fn type_char(&mut self, c: char) {
        if self.finished {
            return;
        }

        let line = &self.source_lines[self.current_line];
        let expected_chars: Vec<char> = line.typeable_content.chars().collect();

        if self.current_col >= expected_chars.len() {
            return;
        }

        let expected = expected_chars[self.current_col];
        let correct = c == expected;

        if self.line_start.is_none() {
            self.stats_at_line_start = (
                self.stats.total_keystrokes,
                self.stats.correct_keystrokes,
                self.stats.accumulated,
            );
            self.line_start = Some(Instant::now());
        }

        self.stats.record_keystroke(correct);
        self.line_keystrokes += 1;
        if correct {
            self.line_correct += 1;
        }

        self.typed_chars[self.current_line][self.current_col] = Some(c);
        self.current_col += 1;
    }

    /// Called when the user presses Enter at the end of a line.
    /// Advances to the next typeable line if all chars are correct.
    pub fn confirm_line(&mut self) {
        if self.finished {
            return;
        }
        // 아직 타이핑 시작 안 한 줄에서 Enter → 건너뛰기
        if self.current_col == 0 && self.line_start.is_none() {
            self.advance_to_next_typeable_line();
            return;
        }
        let expected_len = self.source_lines[self.current_line]
            .typeable_content
            .chars()
            .count();
        if self.current_col >= expected_len && self.current_line_all_correct() {
            self.stats.pause();
            if let Some(start) = self.line_start.take() {
                self.line_stats[self.current_line] = Some(LineStats {
                    keystrokes: self.line_keystrokes,
                    correct_keystrokes: self.line_correct,
                    elapsed: start.elapsed(),
                });
            }
            self.line_keystrokes = 0;
            self.line_correct = 0;
            self.advance_to_next_typeable_line();
        }
    }

    fn current_line_all_correct(&self) -> bool {
        let expected: Vec<char> = self.source_lines[self.current_line]
            .typeable_content
            .chars()
            .collect();
        self.typed_chars[self.current_line]
            .iter()
            .enumerate()
            .all(|(i, typed)| *typed == Some(expected[i]))
    }

    pub fn backspace(&mut self) {
        if self.finished || self.current_col == 0 {
            return;
        }

        self.current_col -= 1;
        self.typed_chars[self.current_line][self.current_col] = None;
    }

    /// Delete one word backwards (Ctrl+W).
    /// Word boundary rules:
    /// - First skip whitespace
    /// - A special character (not alphanumeric, not `_`) is one word by itself
    /// - A run of alphanumeric/`_` characters is one word
    pub fn backspace_word(&mut self) {
        if self.finished || self.current_col == 0 {
            return;
        }

        let expected: Vec<char> = self.source_lines[self.current_line]
            .typeable_content
            .chars()
            .collect();

        // Step 1: skip whitespace
        while self.current_col > 0 && expected[self.current_col - 1] == ' ' {
            self.current_col -= 1;
            self.typed_chars[self.current_line][self.current_col] = None;
        }

        if self.current_col == 0 {
            return;
        }

        let ch = expected[self.current_col - 1];

        if is_word_char(ch) {
            // Step 2a: delete word chars (alphanumeric + _)
            while self.current_col > 0 && is_word_char(expected[self.current_col - 1]) {
                self.current_col -= 1;
                self.typed_chars[self.current_line][self.current_col] = None;
            }
        } else {
            // Step 2b: special char — delete exactly one
            self.current_col -= 1;
            self.typed_chars[self.current_line][self.current_col] = None;
        }
    }

    pub fn restart_line(&mut self) {
        if self.finished {
            return;
        }
        // typed_chars 리셋
        for slot in &mut self.typed_chars[self.current_line] {
            *slot = None;
        }
        self.current_col = 0;
        self.line_keystrokes = 0;
        self.line_correct = 0;
        self.line_start = None;

        // 전역 stats를 이 줄 시작 시점으로 복원
        let (total, correct, accumulated) = self.stats_at_line_start;
        self.stats.total_keystrokes = total;
        self.stats.correct_keystrokes = correct;
        self.stats.accumulated = accumulated;
        self.stats.resume_time = None; // pause 상태
    }

    fn advance_to_next_typeable_line(&mut self) {
        let mut next = self.current_line + 1;
        while next < self.source_lines.len() {
            if self.is_typeable_line(next) {
                self.current_line = next;
                self.current_col = 0;
                return;
            }
            next += 1;
        }

        // No more typeable lines
        self.finished = true;
    }

    pub fn update_scroll(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        let target = self.current_line.saturating_sub(visible_height / 3);
        self.scroll_offset = target;
    }

    pub fn total_typeable_lines(&self) -> usize {
        self.source_lines
            .iter()
            .filter(|sl| matches!(sl.line_type, LineType::Code | LineType::Mixed))
            .count()
    }

    pub fn avg_line_speed(&self) -> f64 {
        let completed: Vec<f64> = self
            .line_stats
            .iter()
            .filter_map(|s| s.as_ref().map(|ls| ls.speed(self.wpm_mode)))
            .collect();
        if completed.is_empty() {
            0.0
        } else {
            completed.iter().sum::<f64>() / completed.len() as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Style;

    fn make_code_line(content: &str) -> SourceLine {
        let len = content.chars().count();
        SourceLine {
            raw: content.to_string(),
            tokens: Vec::new(),
            typeable_content: content.to_string(),
            typeable_styles: vec![Style::default(); len],
            leading_whitespace: String::new(),
            line_type: LineType::Code,
            comment_suffix: None,
        }
    }

    fn make_comment_line(raw: &str) -> SourceLine {
        SourceLine {
            raw: raw.to_string(),
            tokens: Vec::new(),
            typeable_content: String::new(),
            typeable_styles: Vec::new(),
            leading_whitespace: String::new(),
            line_type: LineType::Comment,
            comment_suffix: None,
        }
    }

    fn make_empty_line() -> SourceLine {
        SourceLine {
            raw: String::new(),
            tokens: Vec::new(),
            typeable_content: String::new(),
            typeable_styles: Vec::new(),
            leading_whitespace: String::new(),
            line_type: LineType::Empty,
            comment_suffix: None,
        }
    }

    fn make_mixed_line(code: &str, comment: &str) -> SourceLine {
        let len = code.chars().count();
        SourceLine {
            raw: format!("{code}{comment}"),
            tokens: Vec::new(),
            typeable_content: code.to_string(),
            typeable_styles: vec![Style::default(); len],
            leading_whitespace: String::new(),
            line_type: LineType::Mixed,
            comment_suffix: Some(comment.to_string()),
        }
    }

    #[test]
    fn new_app_starts_at_first_code_line() {
        let lines = vec![make_code_line("hello"), make_code_line("world")];
        let app = App::new(lines, "Test".into(), 1, false, false);
        assert_eq!(app.current_line, 0);
        assert_eq!(app.current_col, 0);
        assert!(!app.finished);
    }

    #[test]
    fn new_app_skips_leading_comments() {
        let lines = vec![
            make_comment_line("// comment"),
            make_empty_line(),
            make_code_line("code"),
        ];
        let app = App::new(lines, "Test".into(), 1, false, false);
        assert_eq!(app.current_line, 2);
        assert!(!app.finished);
    }

    #[test]
    fn new_app_all_comments_is_finished() {
        let lines = vec![
            make_comment_line("// a"),
            make_comment_line("// b"),
            make_empty_line(),
        ];
        let app = App::new(lines, "Test".into(), 1, false, false);
        assert!(app.finished);
    }

    #[test]
    fn type_char_correct() {
        let lines = vec![make_code_line("ab")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        assert_eq!(app.current_col, 1);
        assert_eq!(app.typed_chars[0][0], Some('a'));
        assert_eq!(app.stats.total_keystrokes, 1);
        assert_eq!(app.stats.correct_keystrokes, 1);
    }

    #[test]
    fn type_char_incorrect() {
        let lines = vec![make_code_line("ab")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('x'); // wrong char
        assert_eq!(app.typed_chars[0][0], Some('x'));
        assert_eq!(app.stats.total_keystrokes, 1);
        assert_eq!(app.stats.correct_keystrokes, 0);
    }

    #[test]
    fn type_char_advances_line_on_enter() {
        let lines = vec![make_code_line("a"), make_code_line("b")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        assert_eq!(app.current_line, 0); // not yet advanced
        app.confirm_line();
        assert_eq!(app.current_line, 1);
        assert_eq!(app.current_col, 0);
    }

    #[test]
    fn type_char_finishes_on_last_line() {
        let lines = vec![make_code_line("a")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        assert!(!app.finished); // not finished until Enter
        app.confirm_line();
        assert!(app.finished);
    }

    #[test]
    fn type_char_skips_comment_lines() {
        let lines = vec![
            make_code_line("a"),
            make_comment_line("// skip"),
            make_empty_line(),
            make_code_line("b"),
        ];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.confirm_line();
        assert_eq!(app.current_line, 3); // skipped lines 1 and 2
        assert_eq!(app.current_col, 0);
    }

    #[test]
    fn type_char_when_finished_is_noop() {
        let lines = vec![make_code_line("a")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.confirm_line();
        assert!(app.finished);
        app.type_char('x'); // should be no-op
        assert_eq!(app.stats.total_keystrokes, 1);
    }

    #[test]
    fn backspace_removes_last_char() {
        let lines = vec![make_code_line("ab")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        assert_eq!(app.current_col, 1);
        app.backspace();
        assert_eq!(app.current_col, 0);
        assert_eq!(app.typed_chars[0][0], None);
    }

    #[test]
    fn backspace_at_start_is_noop() {
        let lines = vec![make_code_line("ab")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.backspace(); // at col 0, should be no-op
        assert_eq!(app.current_col, 0);
    }

    #[test]
    fn backspace_when_finished_is_noop() {
        let lines = vec![make_code_line("a")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.confirm_line();
        assert!(app.finished);
        app.backspace();
        assert!(app.finished); // stays finished
    }

    #[test]
    fn total_typeable_lines_counts_code_and_mixed() {
        let lines = vec![
            make_code_line("a"),
            make_comment_line("// b"),
            make_empty_line(),
            make_mixed_line("c;", " // d"),
        ];
        let app = App::new(lines, "Test".into(), 1, false, false);
        assert_eq!(app.total_typeable_lines(), 2);
    }

    #[test]
    fn mixed_line_is_typeable() {
        let lines = vec![make_mixed_line("x = 1;", " // init")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        assert!(!app.finished);
        for c in "x = 1;".chars() {
            app.type_char(c);
        }
        app.confirm_line();
        assert!(app.finished);
    }

    #[test]
    fn update_scroll_keeps_line_in_view() {
        let mut lines = Vec::new();
        for i in 0..50 {
            lines.push(make_code_line(&format!("line {i}")));
        }
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        // Type through first 30 lines
        for _ in 0..30 {
            let line = &app.source_lines[app.current_line].typeable_content.clone();
            for c in line.chars() {
                app.type_char(c);
            }
            app.confirm_line();
        }
        app.update_scroll(20);
        // scroll_offset should be near current_line
        assert!(app.scroll_offset <= app.current_line);
        assert!(app.current_line - app.scroll_offset < 20);
    }

    #[test]
    fn update_scroll_zero_height_noop() {
        let lines = vec![make_code_line("a")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.update_scroll(0);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn typed_chars_initialized_correctly() {
        let lines = vec![
            make_code_line("abc"),
            make_comment_line("// x"),
            make_code_line("de"),
        ];
        let app = App::new(lines, "Test".into(), 1, false, false);
        assert_eq!(app.typed_chars[0].len(), 3);
        assert_eq!(app.typed_chars[1].len(), 0); // comment has no typeable
        assert_eq!(app.typed_chars[2].len(), 2);
    }

    #[test]
    fn typo_blocks_line_advance() {
        let lines = vec![make_code_line("ab"), make_code_line("cd")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.type_char('x'); // wrong char for 'b'
        // Still on line 0 because there's a typo
        assert_eq!(app.current_line, 0);
        assert!(!app.finished);
    }

    #[test]
    fn fix_typo_then_advance() {
        let lines = vec![make_code_line("ab"), make_code_line("cd")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.type_char('x'); // wrong
        app.confirm_line(); // should NOT advance (typo)
        assert_eq!(app.current_line, 0);
        app.backspace();
        app.type_char('b');
        app.confirm_line(); // now correct → advance
        assert_eq!(app.current_line, 1);
    }

    #[test]
    fn all_correct_advances_on_enter() {
        let lines = vec![make_code_line("ab"), make_code_line("cd")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.type_char('b');
        assert_eq!(app.current_line, 0); // not advanced yet
        app.confirm_line();
        assert_eq!(app.current_line, 1); // advanced after Enter
    }

    #[test]
    fn typo_at_start_blocks_advance() {
        let lines = vec![make_code_line("a"), make_code_line("b")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('x'); // wrong
        app.confirm_line(); // should NOT advance
        assert_eq!(app.current_line, 0);
        assert!(!app.finished);
        app.backspace();
        app.type_char('a');
        app.confirm_line();
        assert_eq!(app.current_line, 1);
    }

    #[test]
    fn multiple_typos_must_all_be_fixed() {
        let lines = vec![make_code_line("abc")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('x');
        app.type_char('y');
        app.type_char('c');
        app.confirm_line(); // should NOT advance (2 typos)
        assert_eq!(app.current_line, 0);
        assert!(!app.finished);
        app.backspace();
        app.backspace();
        app.backspace();
        app.type_char('a');
        app.type_char('b');
        app.type_char('c');
        app.confirm_line();
        assert!(app.finished);
    }

    #[test]
    fn confirm_line_mid_line_is_noop() {
        let lines = vec![make_code_line("abc"), make_code_line("def")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.confirm_line(); // only 1 of 3 chars typed
        assert_eq!(app.current_line, 0); // stays
    }

    #[test]
    fn confirm_line_with_typo_is_noop() {
        let lines = vec![make_code_line("ab"), make_code_line("cd")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.type_char('x'); // wrong
        app.confirm_line();
        assert_eq!(app.current_line, 0); // stays because of typo
    }

    #[test]
    fn new_app_empty_source_is_finished() {
        let app = App::new(Vec::new(), "Test".into(), 1, false, false);
        assert!(app.finished);
    }

    #[test]
    fn type_char_at_line_end_with_errors_is_noop() {
        let lines = vec![make_code_line("ab"), make_code_line("cd")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.type_char('x'); // wrong — stuck at end of line
        assert_eq!(app.current_col, 2);
        // Extra chars should be ignored
        app.type_char('z');
        assert_eq!(app.current_col, 2);
        assert_eq!(app.stats.total_keystrokes, 2); // 'z' was not counted
    }

    #[test]
    fn start_line_skips_earlier_lines() {
        let lines = vec![
            make_code_line("aaa"),
            make_code_line("bbb"),
            make_code_line("ccc"),
        ];
        let app = App::new(lines, "Test".into(), 2, false, false); // start at line 2
        assert_eq!(app.current_line, 1); // 0-based index for line 2
    }

    #[test]
    fn start_line_skips_to_next_typeable() {
        let lines = vec![
            make_code_line("aaa"),
            make_comment_line("// skip"),
            make_code_line("ccc"),
        ];
        let app = App::new(lines, "Test".into(), 2, false, false); // line 2 is a comment
        assert_eq!(app.current_line, 2); // jumped to line 3
    }

    #[test]
    fn start_line_beyond_end_is_finished() {
        let lines = vec![make_code_line("aaa")];
        let app = App::new(lines, "Test".into(), 99, false, false);
        assert!(app.finished);
    }

    #[test]
    fn start_line_one_is_default() {
        let lines = vec![make_code_line("aaa"), make_code_line("bbb")];
        let app = App::new(lines, "Test".into(), 1, false, false);
        assert_eq!(app.current_line, 0);
    }

    #[test]
    fn start_line_zero_treated_as_one() {
        let lines = vec![make_code_line("aaa"), make_code_line("bbb")];
        let app = App::new(lines, "Test".into(), 0, false, false);
        assert_eq!(app.current_line, 0);
    }

    #[test]
    fn start_line_finishes_only_remaining() {
        let lines = vec![
            make_code_line("aaa"),
            make_code_line("bbb"),
            make_code_line("ccc"),
        ];
        let mut app = App::new(lines, "Test".into(), 3, false, false); // start at last line
        assert_eq!(app.current_line, 2);
        for c in "ccc".chars() {
            app.type_char(c);
        }
        app.confirm_line();
        assert!(app.finished);
    }

    // --- backspace_word tests ---
    // All tests use a second dummy line to prevent auto-finish when typing the full first line.

    fn type_str(app: &mut App, s: &str) {
        for c in s.chars() {
            app.type_char(c);
        }
    }

    /// Make a two-line app where first line has the given content.
    /// Prevents auto-finish when the first line is fully typed.
    fn make_word_app(content: &str) -> App {
        let lines = vec![make_code_line(content), make_code_line("end")];
        App::new(lines, "Test".into(), 1, false, false)
    }

    #[test]
    fn backspace_word_deletes_identifier() {
        let mut app = make_word_app("let foo_bar");
        type_str(&mut app, "let foo_ba"); // don't finish line
        app.backspace_word();
        assert_eq!(app.current_col, 4); // "let " remains
    }

    #[test]
    fn backspace_word_deletes_word_before_space() {
        let mut app = make_word_app("foo bar baz");
        type_str(&mut app, "foo bar");
        app.backspace_word();
        assert_eq!(app.current_col, 4); // "foo " remains
    }

    #[test]
    fn backspace_word_spaces_then_word() {
        let mut app = make_word_app("x  yz");
        type_str(&mut app, "x  y");
        app.backspace_word();
        assert_eq!(app.current_col, 3); // "x  " (deleted "y")
    }

    #[test]
    fn backspace_word_special_char_single() {
        let mut app = make_word_app("foo; x");
        type_str(&mut app, "foo;");
        app.backspace_word();
        assert_eq!(app.current_col, 3); // "foo"
    }

    #[test]
    fn backspace_word_multiple_special_chars_deletes_one() {
        let mut app = make_word_app("a+= x");
        type_str(&mut app, "a+=");
        app.backspace_word();
        assert_eq!(app.current_col, 2); // "a+"
        app.backspace_word();
        assert_eq!(app.current_col, 1); // "a"
    }

    #[test]
    fn backspace_word_underscore_is_word_char() {
        let mut app = make_word_app("__init x");
        type_str(&mut app, "__init");
        app.backspace_word();
        assert_eq!(app.current_col, 0);
    }

    #[test]
    fn backspace_word_at_start_is_noop() {
        let mut app = make_word_app("hello");
        app.backspace_word();
        assert_eq!(app.current_col, 0);
    }

    #[test]
    fn backspace_word_when_finished_is_noop() {
        let lines = vec![make_code_line("a")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        type_str(&mut app, "a");
        app.confirm_line();
        assert!(app.finished);
        app.backspace_word();
        assert!(app.finished);
    }

    #[test]
    fn backspace_word_skip_spaces_then_word() {
        let mut app = make_word_app("a   bx");
        type_str(&mut app, "a   ");
        // cursor at 4, chars before: "a   "
        app.backspace_word();
        // skip "   " (3 spaces), delete "a" → col=0
        assert_eq!(app.current_col, 0);
    }

    #[test]
    fn backspace_word_paren_after_ident() {
        let mut app = make_word_app("foo(bar) x");
        type_str(&mut app, "foo(bar)");
        app.backspace_word();
        assert_eq!(app.current_col, 7); // deleted ")"
        app.backspace_word();
        assert_eq!(app.current_col, 4); // deleted "bar"
        app.backspace_word();
        assert_eq!(app.current_col, 3); // deleted "("
        app.backspace_word();
        assert_eq!(app.current_col, 0); // deleted "foo"
    }

    #[test]
    fn backspace_word_dot_chain() {
        let mut app = make_word_app("self.name x");
        type_str(&mut app, "self.name");
        app.backspace_word();
        assert_eq!(app.current_col, 5); // "self."
        app.backspace_word();
        assert_eq!(app.current_col, 4); // "self"
        app.backspace_word();
        assert_eq!(app.current_col, 0);
    }

    #[test]
    fn backspace_word_mixed_operator_space() {
        let mut app = make_word_app("x = 42 end");
        type_str(&mut app, "x = 42");
        app.backspace_word();
        assert_eq!(app.current_col, 4); // "x = "
        app.backspace_word();
        assert_eq!(app.current_col, 2); // "x " (skip space, delete "=")
        app.backspace_word();
        assert_eq!(app.current_col, 0); // (skip space, delete "x")
    }

    #[test]
    fn backspace_word_clears_typed_chars() {
        let mut app = make_word_app("ab cd ef");
        type_str(&mut app, "ab cd");
        app.backspace_word();
        assert_eq!(app.current_col, 3); // "ab "
        assert_eq!(app.typed_chars[0][3], None);
        assert_eq!(app.typed_chars[0][4], None);
        assert_eq!(app.typed_chars[0][0], Some('a'));
        assert_eq!(app.typed_chars[0][1], Some('b'));
        assert_eq!(app.typed_chars[0][2], Some(' '));
    }

    #[test]
    fn backspace_word_numbers_are_word_chars() {
        let mut app = make_word_app("var123 x");
        type_str(&mut app, "var123");
        app.backspace_word();
        assert_eq!(app.current_col, 0);
    }

    #[test]
    fn backspace_word_consecutive_specials() {
        // "a!@#" → each special is one word
        let mut app = make_word_app("a!@# x");
        type_str(&mut app, "a!@#");
        app.backspace_word();
        assert_eq!(app.current_col, 3); // deleted "#"
        app.backspace_word();
        assert_eq!(app.current_col, 2); // deleted "@"
        app.backspace_word();
        assert_eq!(app.current_col, 1); // deleted "!"
        app.backspace_word();
        assert_eq!(app.current_col, 0); // deleted "a"
    }

    #[test]
    fn backspace_word_arrow_operator() {
        // "a->b" → Ctrl+W → delete "b" → "a->" → delete ">" → "a-" → delete "-" → "a"
        let mut app = make_word_app("a->b x");
        type_str(&mut app, "a->b");
        app.backspace_word();
        assert_eq!(app.current_col, 3); // "a->"
        app.backspace_word();
        assert_eq!(app.current_col, 2); // "a-"
        app.backspace_word();
        assert_eq!(app.current_col, 1); // "a"
    }

    #[test]
    fn backspace_word_space_only_typed() {
        let mut app = make_word_app("    x");
        type_str(&mut app, "    "); // 4 spaces
        app.backspace_word();
        // skip all 4 spaces → col=0
        assert_eq!(app.current_col, 0);
    }

    #[test]
    fn backspace_word_fn_signature() {
        // "fn a(v: &i32)" → Ctrl+W repeatedly
        let mut app = make_word_app("fn a(v: &i32) x");
        type_str(&mut app, "fn a(v: &i32)");
        // col=13
        app.backspace_word(); // delete ")"
        assert_eq!(app.current_col, 12);
        app.backspace_word(); // delete "i32"
        assert_eq!(app.current_col, 9);
        app.backspace_word(); // delete "&"
        assert_eq!(app.current_col, 8);
        app.backspace_word(); // skip " ", delete ":"
        assert_eq!(app.current_col, 6);
        app.backspace_word(); // delete "v"
        assert_eq!(app.current_col, 5);
        app.backspace_word(); // delete "("
        assert_eq!(app.current_col, 4);
        app.backspace_word(); // delete "a"
        assert_eq!(app.current_col, 3);
        app.backspace_word(); // skip " ", delete "fn"
        assert_eq!(app.current_col, 0);
    }

    // --- coverage gap tests ---

    #[test]
    fn start_at_comment_only_remaining_is_finished() {
        // Lines: code, comment, comment. Start at line 2 (comment).
        // advance finds no typeable lines → finished = true (covers line 67)
        let lines = vec![
            make_code_line("a"),
            make_comment_line("// b"),
            make_comment_line("// c"),
        ];
        let app = App::new(lines, "Test".into(), 2, false, false);
        assert!(app.finished);
    }

    #[test]
    fn confirm_line_when_finished_is_noop() {
        // covers line 142
        let lines = vec![make_code_line("a")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.confirm_line();
        assert!(app.finished);
        app.confirm_line(); // second call → early return
        assert!(app.finished);
    }

    #[test]
    fn confirm_line_records_line_stats() {
        let lines = vec![make_code_line("ab"), make_code_line("cd")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.type_char('b');
        app.confirm_line();
        assert!(app.line_stats[0].is_some());
        let ls = app.line_stats[0].as_ref().unwrap();
        assert_eq!(ls.keystrokes, 2);
        assert_eq!(ls.correct_keystrokes, 2);
        assert!(ls.elapsed.as_nanos() > 0);
        assert!(ls.kpm() > 0.0);
        assert!(app.line_stats[1].is_none());
    }

    #[test]
    fn avg_line_kpm_no_completed() {
        let lines = vec![make_code_line("a")];
        let app = App::new(lines, "Test".into(), 1, false, false);
        assert_eq!(app.avg_line_speed(), 0.0);
    }

    #[test]
    fn avg_line_kpm_with_completed() {
        let lines = vec![
            make_code_line("ab"),
            make_code_line("cd"),
            make_code_line("ef"),
        ];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        // Complete first line
        app.type_char('a');
        app.type_char('b');
        app.confirm_line();
        // Complete second line
        app.type_char('c');
        app.type_char('d');
        app.confirm_line();
        let avg = app.avg_line_speed();
        assert!(avg > 0.0);
    }

    #[test]
    fn enter_skips_untyped_line() {
        let lines = vec![make_code_line("ab"), make_code_line("cd")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        assert_eq!(app.current_line, 0);
        // Enter without typing → skip to next line
        app.confirm_line();
        assert_eq!(app.current_line, 1);
        assert!(app.line_stats[0].is_none()); // no stats recorded
    }

    #[test]
    fn enter_does_not_skip_partially_typed_line() {
        let lines = vec![make_code_line("ab"), make_code_line("cd")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.confirm_line(); // incomplete → should not skip
        assert_eq!(app.current_line, 0);
    }

    #[test]
    fn restart_line_resets_typed_chars() {
        let lines = vec![make_code_line("ab"), make_code_line("cd")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.type_char('x');
        app.restart_line();
        assert_eq!(app.current_col, 0);
        assert!(app.typed_chars[0].iter().all(|c| c.is_none()));
    }

    #[test]
    fn restart_line_restores_stats() {
        let lines = vec![make_code_line("ab"), make_code_line("cd")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        assert_eq!(app.stats.total_keystrokes, 0);
        app.type_char('a');
        app.type_char('x');
        assert_eq!(app.stats.total_keystrokes, 2);
        app.restart_line();
        assert_eq!(app.stats.total_keystrokes, 0);
        assert!(app.stats.resume_time.is_none()); // paused
    }

    #[test]
    fn restart_line_when_finished_is_noop() {
        let lines = vec![make_code_line("a")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.type_char('a');
        app.confirm_line();
        assert!(app.finished);
        app.restart_line(); // should not panic
        assert!(app.finished);
    }

    #[test]
    fn enter_skip_on_last_line_finishes() {
        let lines = vec![make_code_line("a")];
        let mut app = App::new(lines, "Test".into(), 1, false, false);
        app.confirm_line(); // skip only line
        assert!(app.finished);
    }
}
