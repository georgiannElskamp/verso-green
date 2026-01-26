//! Frame Pacing
//!
//! This module provides frame pacing to align composites with display
//! refresh rate, reducing tearing and improving visual smoothness.

use std::time::{Duration, Instant};

/// Frame pacing configuration
#[derive(Clone, Debug)]
pub struct FramePacingConfig {
    /// Target refresh rate in Hz (0 = auto-detect)
    pub target_refresh_hz: f64,
    /// Enable adaptive vsync (skip frames when behind)
    pub adaptive_vsync: bool,
    /// Number of frames to average for timing calculations
    pub averaging_window: u32,
    /// Threshold for frame drop detection (multiplier of target frame time)
    pub frame_drop_threshold: f32,
}

impl Default for FramePacingConfig {
    fn default() -> Self {
        Self {
            target_refresh_hz: 60.0,
            adaptive_vsync: true,
            averaging_window: 30,
            frame_drop_threshold: 1.5,
        }
    }
}

/// Frame timing and pacing state
#[derive(Debug)]
pub struct FramePacing {
    config: FramePacingConfig,
    /// Target frame duration based on display refresh rate
    target_frame_duration: Duration,
    /// Timestamp of last frame presentation
    last_frame_time: Instant,
    /// Rolling average frame time
    avg_frame_time: Duration,
    /// Frame time history for averaging
    frame_time_history: Vec<Duration>,
    /// Total frames rendered
    frame_count: u64,
    /// Frames dropped (took longer than threshold)
    frames_dropped: u64,
    /// Whether we're currently behind schedule
    behind_schedule: bool,
}

impl FramePacing {
    /// Create a new frame pacer with the given configuration
    pub fn new(config: FramePacingConfig) -> Self {
        let target_frame_duration = Duration::from_secs_f64(1.0 / config.target_refresh_hz);

        Self {
            target_frame_duration,
            last_frame_time: Instant::now(),
            avg_frame_time: target_frame_duration,
            frame_time_history: Vec::with_capacity(config.averaging_window as usize),
            frame_count: 0,
            frames_dropped: 0,
            behind_schedule: false,
            config,
        }
    }

    /// Set target refresh rate (call when display changes)
    pub fn set_target_refresh_rate(&mut self, hz: f64) {
        self.config.target_refresh_hz = hz;
        self.target_frame_duration = Duration::from_secs_f64(1.0 / hz);
        log::info!("Frame pacing: target refresh rate set to {:.1}Hz", hz);
    }

    /// Get target refresh rate
    pub fn target_refresh_rate(&self) -> f64 {
        self.config.target_refresh_hz
    }

    /// Get target frame duration
    pub fn target_frame_duration(&self) -> Duration {
        self.target_frame_duration
    }

    /// Check if it's time for a new frame
    pub fn should_generate_frame(&self) -> bool {
        let elapsed = self.last_frame_time.elapsed();

        if self.config.adaptive_vsync && self.behind_schedule {
            // If behind schedule, generate frame immediately
            return true;
        }

        elapsed >= self.target_frame_duration
    }

    /// Get time until next frame should be generated
    pub fn time_until_next_frame(&self) -> Duration {
        let elapsed = self.last_frame_time.elapsed();
        self.target_frame_duration.saturating_sub(elapsed)
    }

    /// Mark that a frame has been presented
    pub fn on_frame_presented(&mut self) {
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_frame_time);

        // Update frame time history
        self.frame_time_history.push(frame_time);
        if self.frame_time_history.len() > self.config.averaging_window as usize {
            self.frame_time_history.remove(0);
        }

        // Calculate rolling average
        if !self.frame_time_history.is_empty() {
            let sum: Duration = self.frame_time_history.iter().sum();
            self.avg_frame_time = sum / self.frame_time_history.len() as u32;
        }

        // Check for frame drops
        let drop_threshold =
            self.target_frame_duration.mul_f32(self.config.frame_drop_threshold);
        if frame_time > drop_threshold {
            self.frames_dropped += 1;
            self.behind_schedule = true;
            log::debug!(
                "Frame drop detected: {:?} > {:?} (frame {})",
                frame_time,
                self.target_frame_duration,
                self.frame_count
            );
        } else {
            self.behind_schedule = false;
        }

        self.last_frame_time = now;
        self.frame_count += 1;
    }

    /// Mark that a frame was skipped (not generated)
    pub fn on_frame_skipped(&mut self) {
        // Don't update timing, just note that we skipped
        log::trace!("Frame skipped at count {}", self.frame_count);
    }

    /// Get frame statistics
    pub fn stats(&self) -> FramePacingStats {
        FramePacingStats {
            frame_count: self.frame_count,
            frames_dropped: self.frames_dropped,
            avg_frame_time: self.avg_frame_time,
            target_frame_time: self.target_frame_duration,
            current_fps: 1.0 / self.avg_frame_time.as_secs_f64(),
            target_fps: self.config.target_refresh_hz,
            behind_schedule: self.behind_schedule,
        }
    }

    /// Reset statistics (useful when display changes)
    pub fn reset_stats(&mut self) {
        self.frame_count = 0;
        self.frames_dropped = 0;
        self.frame_time_history.clear();
        self.avg_frame_time = self.target_frame_duration;
        self.behind_schedule = false;
    }
}

impl Default for FramePacing {
    fn default() -> Self {
        Self::new(FramePacingConfig::default())
    }
}

/// Frame pacing statistics
#[derive(Clone, Debug)]
pub struct FramePacingStats {
    /// Total frames rendered
    pub frame_count: u64,
    /// Frames that took longer than threshold
    pub frames_dropped: u64,
    /// Average frame time
    pub avg_frame_time: Duration,
    /// Target frame time
    pub target_frame_time: Duration,
    /// Current effective FPS
    pub current_fps: f64,
    /// Target FPS
    pub target_fps: f64,
    /// Whether currently behind schedule
    pub behind_schedule: bool,
}

impl FramePacingStats {
    /// Calculate frame drop percentage
    pub fn drop_percentage(&self) -> f64 {
        if self.frame_count == 0 {
            0.0
        } else {
            (self.frames_dropped as f64 / self.frame_count as f64) * 100.0
        }
    }

    /// Check if performance is acceptable (< 5% drops)
    pub fn is_acceptable(&self) -> bool {
        self.drop_percentage() < 5.0
    }
}

/// Vsync mode for frame presentation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VsyncMode {
    /// Vsync disabled - present immediately
    Off,
    /// Standard vsync - wait for vertical blank
    On,
    /// Adaptive vsync - vsync when ahead, tear when behind
    Adaptive,
    /// Mailbox - always use latest frame, no tearing
    Mailbox,
}

impl Default for VsyncMode {
    fn default() -> Self {
        Self::Adaptive
    }
}

/// Helper to detect display refresh rate
pub fn detect_refresh_rate(monitor_refresh_millihertz: Option<u32>) -> f64 {
    monitor_refresh_millihertz
        .map(|mhz| mhz as f64 / 1000.0)
        .unwrap_or(60.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_default_config() {
        let config = FramePacingConfig::default();
        assert_eq!(config.target_refresh_hz, 60.0);
        assert!(config.adaptive_vsync);
    }

    #[test]
    fn test_frame_duration_calculation() {
        let pacing = FramePacing::default();
        let expected = Duration::from_secs_f64(1.0 / 60.0);
        let diff = if pacing.target_frame_duration > expected {
            pacing.target_frame_duration - expected
        } else {
            expected - pacing.target_frame_duration
        };
        assert!(diff < Duration::from_micros(100));
    }

    #[test]
    fn test_refresh_rate_change() {
        let mut pacing = FramePacing::default();
        pacing.set_target_refresh_rate(144.0);

        let expected = Duration::from_secs_f64(1.0 / 144.0);
        let diff = if pacing.target_frame_duration > expected {
            pacing.target_frame_duration - expected
        } else {
            expected - pacing.target_frame_duration
        };
        assert!(diff < Duration::from_micros(100));
    }

    #[test]
    fn test_should_generate_frame() {
        let mut pacing = FramePacing::default();

        // Immediately after creation, should not generate
        // (depending on timing, might be true if test runs slow)

        // After waiting longer than frame duration, should generate
        thread::sleep(Duration::from_millis(20)); // > 16.67ms
        assert!(pacing.should_generate_frame());
    }

    #[test]
    fn test_stats_calculation() {
        let mut pacing = FramePacing::default();

        // Simulate some frames
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(16));
            pacing.on_frame_presented();
        }

        let stats = pacing.stats();
        assert_eq!(stats.frame_count, 10);
        assert!(stats.current_fps > 0.0);
    }

    #[test]
    fn test_detect_refresh_rate() {
        assert_eq!(detect_refresh_rate(Some(60000)), 60.0);
        assert_eq!(detect_refresh_rate(Some(144000)), 144.0);
        assert_eq!(detect_refresh_rate(None), 60.0); // Default fallback
    }
}
