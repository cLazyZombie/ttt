use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct LineStats {
    pub keystrokes: usize,
    #[allow(dead_code)]
    pub correct_keystrokes: usize,
    pub elapsed: Duration,
}

impl LineStats {
    pub fn kpm(&self) -> f64 {
        let minutes = self.elapsed.as_secs_f64() / 60.0;
        if minutes > 0.0 {
            self.keystrokes as f64 / minutes
        } else {
            0.0
        }
    }

    pub fn wpm(&self) -> f64 {
        self.kpm() / 5.0
    }

    pub fn speed(&self, wpm_mode: bool) -> f64 {
        if wpm_mode { self.wpm() } else { self.kpm() }
    }
}

pub struct Stats {
    pub total_keystrokes: usize,
    pub correct_keystrokes: usize,
    pub accumulated: Duration,
    pub resume_time: Option<Instant>,
}

impl Stats {
    pub fn new() -> Self {
        Stats {
            total_keystrokes: 0,
            correct_keystrokes: 0,
            accumulated: Duration::ZERO,
            resume_time: None,
        }
    }

    pub fn pause(&mut self) {
        if let Some(t) = self.resume_time.take() {
            self.accumulated += t.elapsed();
        }
    }

    pub fn resume(&mut self) {
        if self.resume_time.is_none() {
            self.resume_time = Some(Instant::now());
        }
    }

    pub fn record_keystroke(&mut self, correct: bool) {
        self.resume();
        self.total_keystrokes += 1;
        if correct {
            self.correct_keystrokes += 1;
        }
    }

    pub fn elapsed_secs(&self) -> f64 {
        let current = self
            .resume_time
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO);
        (self.accumulated + current).as_secs_f64()
    }

    pub fn kpm(&self) -> f64 {
        let minutes = self.elapsed_secs() / 60.0;
        if minutes > 0.0 {
            self.total_keystrokes as f64 / minutes
        } else {
            0.0
        }
    }

    pub fn wpm(&self) -> f64 {
        self.kpm() / 5.0
    }

    pub fn speed(&self, wpm_mode: bool) -> f64 {
        if wpm_mode { self.wpm() } else { self.kpm() }
    }

    pub fn accuracy(&self) -> f64 {
        if self.total_keystrokes > 0 {
            (self.correct_keystrokes as f64 / self.total_keystrokes as f64) * 100.0
        } else {
            100.0
        }
    }

    pub fn elapsed_display(&self) -> String {
        let secs = self.elapsed_secs() as u64;
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stats_defaults() {
        let stats = Stats::new();
        assert_eq!(stats.total_keystrokes, 0);
        assert_eq!(stats.correct_keystrokes, 0);
        assert_eq!(stats.accumulated, Duration::ZERO);
        assert!(stats.resume_time.is_none());
    }

    #[test]
    fn accuracy_no_keystrokes_is_100() {
        let stats = Stats::new();
        assert_eq!(stats.accuracy(), 100.0);
    }

    #[test]
    fn accuracy_all_correct() {
        let mut stats = Stats::new();
        stats.record_keystroke(true);
        stats.record_keystroke(true);
        stats.record_keystroke(true);
        assert_eq!(stats.accuracy(), 100.0);
    }

    #[test]
    fn accuracy_mixed() {
        let mut stats = Stats::new();
        stats.record_keystroke(true);
        stats.record_keystroke(false);
        assert_eq!(stats.accuracy(), 50.0);
    }

    #[test]
    fn accuracy_all_wrong() {
        let mut stats = Stats::new();
        stats.record_keystroke(false);
        stats.record_keystroke(false);
        assert_eq!(stats.accuracy(), 0.0);
    }

    #[test]
    fn record_keystroke_starts_timer() {
        let mut stats = Stats::new();
        assert!(stats.resume_time.is_none());
        stats.record_keystroke(true);
        assert!(stats.resume_time.is_some());
    }

    #[test]
    fn record_keystroke_counts() {
        let mut stats = Stats::new();
        stats.record_keystroke(true);
        stats.record_keystroke(false);
        stats.record_keystroke(true);
        assert_eq!(stats.total_keystrokes, 3);
        assert_eq!(stats.correct_keystrokes, 2);
    }

    #[test]
    fn kpm_zero_before_start() {
        let stats = Stats::new();
        assert_eq!(stats.kpm(), 0.0);
    }

    #[test]
    fn elapsed_secs_zero_before_start() {
        let stats = Stats::new();
        assert_eq!(stats.elapsed_secs(), 0.0);
    }

    #[test]
    fn elapsed_display_zero() {
        let stats = Stats::new();
        assert_eq!(stats.elapsed_display(), "0:00");
    }

    #[test]
    fn elapsed_display_format() {
        let mut stats = Stats::new();
        stats.accumulated = Duration::from_secs(125);
        assert_eq!(stats.elapsed_display(), "2:05");
    }

    #[test]
    fn kpm_with_elapsed_time() {
        let mut stats = Stats::new();
        // Simulate: 120 keystrokes over 60 seconds = 120 KPM
        stats.total_keystrokes = 120;
        stats.accumulated = Duration::from_secs(60);
        let kpm = stats.kpm();
        assert!((kpm - 120.0).abs() < 5.0, "Expected ~120 KPM, got {kpm}");
    }

    #[test]
    fn pause_stops_elapsed() {
        let mut stats = Stats::new();
        stats.resume();
        std::thread::sleep(Duration::from_millis(50));
        stats.pause();
        let elapsed = stats.elapsed_secs();
        std::thread::sleep(Duration::from_millis(50));
        let after = stats.elapsed_secs();
        assert!(
            (elapsed - after).abs() < 0.001,
            "Time should not advance while paused"
        );
    }

    #[test]
    fn resume_continues_elapsed() {
        let mut stats = Stats::new();
        stats.accumulated = Duration::from_secs(10);
        stats.resume();
        std::thread::sleep(Duration::from_millis(50));
        assert!(stats.elapsed_secs() > 10.0);
    }

    #[test]
    fn line_stats_kpm_basic() {
        let ls = LineStats {
            keystrokes: 60,
            correct_keystrokes: 58,
            elapsed: Duration::from_secs(30),
        };
        // 60 keystrokes in 30 seconds = 120 KPM
        assert!((ls.kpm() - 120.0).abs() < 1.0);
    }

    #[test]
    fn line_stats_kpm_zero_elapsed() {
        let ls = LineStats {
            keystrokes: 10,
            correct_keystrokes: 10,
            elapsed: Duration::ZERO,
        };
        assert_eq!(ls.kpm(), 0.0);
    }

    #[test]
    fn line_stats_wpm() {
        let ls = LineStats {
            keystrokes: 60,
            correct_keystrokes: 60,
            elapsed: Duration::from_secs(30),
        };
        // 120 KPM / 5 = 24 WPM
        assert!((ls.wpm() - 24.0).abs() < 1.0);
    }

    #[test]
    fn line_stats_speed_kpm_mode() {
        let ls = LineStats {
            keystrokes: 60,
            correct_keystrokes: 60,
            elapsed: Duration::from_secs(30),
        };
        assert!((ls.speed(false) - 120.0).abs() < 1.0);
    }

    #[test]
    fn line_stats_speed_wpm_mode() {
        let ls = LineStats {
            keystrokes: 60,
            correct_keystrokes: 60,
            elapsed: Duration::from_secs(30),
        };
        assert!((ls.speed(true) - 24.0).abs() < 1.0);
    }

    #[test]
    fn stats_wpm() {
        let mut stats = Stats::new();
        stats.total_keystrokes = 120;
        stats.accumulated = Duration::from_secs(60);
        // 120 KPM / 5 = 24 WPM
        assert!((stats.wpm() - 24.0).abs() < 1.0);
    }

    #[test]
    fn stats_speed_kpm_mode() {
        let mut stats = Stats::new();
        stats.total_keystrokes = 120;
        stats.accumulated = Duration::from_secs(60);
        assert!((stats.speed(false) - 120.0).abs() < 5.0);
    }

    #[test]
    fn stats_speed_wpm_mode() {
        let mut stats = Stats::new();
        stats.total_keystrokes = 120;
        stats.accumulated = Duration::from_secs(60);
        assert!((stats.speed(true) - 24.0).abs() < 1.0);
    }
}
