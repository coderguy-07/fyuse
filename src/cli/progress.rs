/// Progress indicators for long-running operations
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Progress bar for downloads and long-running operations
pub struct ProgressBar {
    total: u64,
    current: Arc<AtomicU64>,
    message: Arc<RwLock<String>>,
    start_time: Instant,
    finished: Arc<AtomicBool>,
}

impl ProgressBar {
    /// Create a new progress bar
    pub fn new(total: u64, message: impl Into<String>) -> Self {
        Self {
            total,
            current: Arc::new(AtomicU64::new(0)),
            message: Arc::new(RwLock::new(message.into())),
            start_time: Instant::now(),
            finished: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Update progress
    pub fn set_progress(&self, current: u64) {
        self.current.store(current, Ordering::Relaxed);
        self.render();
    }

    /// Increment progress
    pub fn inc(&self, delta: u64) {
        self.current.fetch_add(delta, Ordering::Relaxed);
        self.render();
    }

    /// Update message
    pub async fn set_message(&self, message: impl Into<String>) {
        let mut msg = self.message.write().await;
        *msg = message.into();
        drop(msg);
        self.render();
    }

    /// Mark as finished
    pub fn finish(&self) {
        self.finished.store(true, Ordering::Relaxed);
        self.render();
        println!(); // New line after completion
    }

    /// Render the progress bar
    fn render(&self) {
        if self.finished.load(Ordering::Relaxed) {
            return;
        }

        let current = self.current.load(Ordering::Relaxed);
        let percentage = if self.total > 0 {
            (current as f64 / self.total as f64 * 100.0) as u8
        } else {
            0
        };

        let elapsed = self.start_time.elapsed();
        let speed = if elapsed.as_secs() > 0 {
            current / elapsed.as_secs()
        } else {
            0
        };

        let eta = if speed > 0 && self.total > current {
            let remaining = self.total - current;
            Duration::from_secs(remaining / speed)
        } else {
            Duration::from_secs(0)
        };

        // Create progress bar visualization
        let bar_width = 40;
        let filled = (bar_width as f64 * percentage as f64 / 100.0) as usize;
        let empty = bar_width - filled;

        let bar = format!("[{}{}]", "=".repeat(filled), " ".repeat(empty));

        // Format sizes
        let current_str = format_bytes(current);
        let total_str = format_bytes(self.total);
        let speed_str = format_bytes(speed);

        print!(
            "\r{} {} {}/{} ({}/s) ETA: {}s",
            bar,
            percentage,
            current_str,
            total_str,
            speed_str,
            eta.as_secs()
        );
        io::stdout().flush().ok();
    }
}

/// Spinner for indeterminate operations
pub struct Spinner {
    message: String,
    frames: Vec<&'static str>,
    finished: Arc<AtomicBool>,
}

impl Spinner {
    /// Create a new spinner
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            finished: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the spinner
    pub async fn start(self) -> SpinnerHandle {
        let finished = self.finished.clone();
        let message = self.message.clone();
        let frames = self.frames.clone();

        let handle = tokio::spawn(async move {
            let mut current_frame = 0;
            while !finished.load(Ordering::Relaxed) {
                print!("\r{} {}", frames[current_frame], message);
                io::stdout().flush().ok();
                tokio::time::sleep(Duration::from_millis(80)).await;
                current_frame = (current_frame + 1) % frames.len();
            }
            // Clear the line
            print!("\r{}\r", " ".repeat(80));
            io::stdout().flush().ok();
        });

        SpinnerHandle {
            finished: self.finished,
            handle,
        }
    }
}

/// Handle for controlling a spinner
pub struct SpinnerHandle {
    finished: Arc<AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
}

impl SpinnerHandle {
    /// Stop the spinner with a success message
    pub async fn finish_with_message(self, message: impl Into<String>) {
        self.finished.store(true, Ordering::Relaxed);
        self.handle.await.ok();
        println!("✓ {}", message.into());
    }

    /// Stop the spinner with an error message
    pub async fn finish_with_error(self, message: impl Into<String>) {
        self.finished.store(true, Ordering::Relaxed);
        self.handle.await.ok();
        eprintln!("✗ {}", message.into());
    }

    /// Stop the spinner
    pub async fn finish(self) {
        self.finished.store(true, Ordering::Relaxed);
        self.handle.await.ok();
    }
}

/// Format bytes into human-readable format
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Simple progress indicator for operations with known steps
pub struct StepProgress {
    total_steps: usize,
    current_step: usize,
    step_name: String,
}

impl StepProgress {
    /// Create a new step progress indicator
    pub fn new(total_steps: usize) -> Self {
        Self {
            total_steps,
            current_step: 0,
            step_name: String::new(),
        }
    }

    /// Start a new step
    pub fn start_step(&mut self, step_name: impl Into<String>) {
        self.current_step += 1;
        self.step_name = step_name.into();
        println!(
            "[{}/{}] {}...",
            self.current_step, self.total_steps, self.step_name
        );
    }

    /// Complete the current step
    pub fn complete_step(&self) {
        println!("  ✓ {} completed", self.step_name);
    }

    /// Fail the current step
    pub fn fail_step(&self, error: impl Into<String>) {
        eprintln!("  ✗ {} failed: {}", self.step_name, error.into());
    }

    /// Check if all steps are complete
    pub fn is_complete(&self) -> bool {
        self.current_step >= self.total_steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512.00 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_bytes(1536), "1.50 KB");
    }

    #[test]
    fn test_step_progress() {
        let mut progress = StepProgress::new(3);
        assert!(!progress.is_complete());

        progress.start_step("Step 1");
        assert!(!progress.is_complete());

        progress.start_step("Step 2");
        assert!(!progress.is_complete());

        progress.start_step("Step 3");
        assert!(progress.is_complete());
    }

    #[tokio::test]
    async fn test_progress_bar() {
        let progress = ProgressBar::new(100, "Testing");
        progress.set_progress(50);
        assert_eq!(progress.current.load(Ordering::Relaxed), 50);

        progress.inc(25);
        assert_eq!(progress.current.load(Ordering::Relaxed), 75);

        progress.finish();
        assert!(progress.finished.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_spinner() {
        let spinner = Spinner::new("Testing");
        let handle = spinner.start().await;

        tokio::time::sleep(Duration::from_millis(200)).await;
        handle.finish_with_message("Done").await;
    }
}
