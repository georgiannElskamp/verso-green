//! Frame Pacing
//!
//! This module provides frame timing and pacing to align rendering
//! with display refresh rates, reducing tearing and optimizing latency.

use std::time::{Duration, Instant};

/// Default refresh rate assumption when detection fails
const DEFAULT_REFRESH_RATE_HZ: f64 = 60.0;

/// Frame timing and pacing state
#[derive(Debug)]
pub struct FramePacing {
    /// Target frame duration based on display refresh rate
    target_frame_duration: Duration,
    /// Current refresh rate in Hz
    refresh_rate_hz: f64,
    /// Timestamp of last frame presentation
    last_frame_time: Instant,
    /// Timestamp when last frame started rendering
    last_render_start: Option<Instant>,
    /// Rolling average frame time
    avg_frame_time: Duration,
    /// Rolling average render time (excluding vsync wait)
    avg_render_time: Duration,
    /// Total frames rendered
    frame_count: u64,
    /// Frames that exceeded budget
    dropped_frames: u64,
    /// Configuration
    config: FramePacingConfig,
}

/// Configuration for frame pacing behavior
#[derive(Clone, Debug)]
pub struct FramePacingConfig {
    /// Enable vsync-aligned frame scheduling
    pub vsync_enabled: bool,
    /// Enable adaptive vsync (skip vsync when behind)
    pub adaptive_vsync: bool,
    /// Frame time smoothing factor (0.0-1.0, higher = more smoothing)
    pub smoothing_factor: f64,
    /// Threshold for frame drop detection (multiplier of target duration)
    pub drop_threshold: f64,
    /// Enable frame timing debug logging
    pub debug_logging: bool,
}

impl Default for FramePacingConfig {
    fn default() -> Self {
        Self {
            vsync_enabled: true,
            adaptive_vsync: true,
            smoothing_factor: 0.1,
            drop_threshold: 1.5,
            debug_logging: false,
        }
    }
}

/// Frame timing statistics
#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    /// Total frames rendered
    pub total_frames: u64,
    /// Frames that exceeded budget
    pub dropped_frames: u64,
    /// Average frame time
    pub avg_frame_time_ms: f64,
    /// Average render time (excluding vsync wait)
    pub avg_render_time_ms: f64,
    /// Current refresh rate
    pub refresh_rate_hz: f64,
    /// Effective FPS
    pub effective_fps: f64,
}

impl FramePacing {
    /// Create a new frame pacing controller with default settings
    pub fn new() -> Self {
        Self::with_config(FramePacingConfig::default())
    }

    /// Create a new frame pacing controller with custom configuration
    pub fn with_config(config: FramePacingConfig) -> Self {
        let target = Duration::from_secs_f64(1.0 / DEFAULT_REFRESH_RATE_HZ);
        Self {
            target_frame_duration: target,
            refresh_rate_hz: DEFAULT_REFRESH_RATE_HZ,
            last_frame_time: Instant::now(),
            last_render_start: None,
            avg_frame_time: target,
            avg_render_time: Duration::ZERO,
            frame_count: 0,
            dropped_frames: 0,
            config,
        }
    }

    /// Set target refresh rate
    pub fn set_refresh_rate(&mut self, hz: f64) {
        if hz > 0.0 && hz <= 360.0 {
            self.refresh_rate_hz = hz;
            self.target_frame_duration = Duration::from_secs_f64(1.0 / hz);
            log::info!("Frame pacing: target refresh rate set to {:.1}Hz", hz);
        } else {
            log::warn!("Invalid refresh rate {:.1}Hz, keeping {:.1}Hz", hz, self.refresh_rate_hz);
        }
    }

    /// Get current refresh rate
    pub fn refresh_rate(&self) -> f64 {
        self.refresh_rate_hz
    }

    /// Get target frame duration
    pub fn target_frame_duration(&self) -> Duration {
        self.target_frame_duration
    }

    /// Check if it's time to generate a new frame
    pub fn should_generate_frame(&self) -> bool {
        if !self.config.vsync_enabled {
            return true;
        }

        let elapsed = self.last_frame_time.elapsed();

        // Adaptive vsync: if we're significantly behind, skip vsync wait
        if self.config.adaptive_vsync {
            let behind_threshold = self.target_frame_duration.mul_f64(self.config.drop_threshold);
            if elapsed >= behind_threshold {
                return true;
            }
        }

        elapsed >= self.target_frame_duration
    }

    /// Get time until next frame should be generated
    pub fn time_until_next_frame(&self) -> Duration {
        if !self.config.vsync_enabled {
            return Duration::ZERO;
        }

        let elapsed = self.last_frame_time.elapsed();
        self.target_frame_duration.saturating_sub(elapsed)
    }

    /// Mark the start of frame rendering
    pub fn begin_frame(&mut self) {
        self.last_render_start = Some(Instant::now());
    }

    /// Mark frame presentation and update timing
    pub fn on_frame_presented(&mut self) {
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_frame_time);

        // Update render time if we tracked begin_frame
        if let Some(render_start) = self.last_render_start.take() {
            let render_time = now.duration_since(render_start);
            self.avg_render_time = self.smooth_duration(self.avg_render_time, render_time);
        }

        // Detect frame drops
        let drop_threshold = self.target_frame_duration.mul_f64(self.config.drop_threshold);
        if frame_time > drop_threshold {
            self.dropped_frames += 1;
            if self.config.debug_logging {
                log::debug!(
                    "Frame drop detected: {:.2}ms > {:.2}ms threshold",
                    frame_time.as_secs_f64() * 1000.0,
                    drop_threshold.as_secs_f64() * 1000.0
                );
            }
        }

        // Update averages
        self.avg_frame_time = self.smooth_duration(self.avg_frame_time, frame_time);
        self.last_frame_time = now;
        self.frame_count += 1;

        if self.config.debug_logging && self.frame_count % 60 == 0 {
            log::debug!(
                "Frame timing: avg={:.2}ms, render={:.2}ms, drops={}",
                self.avg_frame_time.as_secs_f64() * 1000.0,
                self.avg_render_time.as_secs_f64() * 1000.0,
                self.dropped_frames
            );
        }
    }

    /// Get current frame statistics
    pub fn stats(&self) -> FrameStats {
        let effective_fps = if self.avg_frame_time.as_secs_f64() > 0.0 {
            1.0 / self.avg_frame_time.as_secs_f64()
        } else {
            0.0
        };

        FrameStats {
            total_frames: self.frame_count,
            dropped_frames: self.dropped_frames,
            avg_frame_time_ms: self.avg_frame_time.as_secs_f64() * 1000.0,
            avg_render_time_ms: self.avg_render_time.as_secs_f64() * 1000.0,
            refresh_rate_hz: self.refresh_rate_hz,
            effective_fps,
        }
    }

    /// Get frame drop percentage
    pub fn drop_percentage(&self) -> f64 {
        if self.frame_count == 0 {
            return 0.0;
        }
        (self.dropped_frames as f64 / self.frame_count as f64) * 100.0
    }

    /// Check if rendering is keeping up with target refresh rate
    pub fn is_keeping_up(&self) -> bool {
        self.avg_frame_time <= self.target_frame_duration.mul_f64(1.1)
    }

    /// Get mutable reference to configuration
    pub fn config_mut(&mut self) -> &mut FramePacingConfig {
        &mut self.config
    }

    /// Reset timing statistics
    pub fn reset_stats(&mut self) {
        self.frame_count = 0;
        self.dropped_frames = 0;
        self.avg_frame_time = self.target_frame_duration;
        self.avg_render_time = Duration::ZERO;
    }

    /// Apply exponential smoothing to duration
    fn smooth_duration(&self, current: Duration, new: Duration) -> Duration {
        let alpha = self.config.smoothing_factor;
        Duration::from_secs_f64(
            current.as_secs_f64() * (1.0 - alpha) + new.as_secs_f64() * alpha,
        )
    }
}

impl Default for FramePacing {
    fn default() -> Self {
        Self::new()
    }
}

/// Monitor refresh rate detection utilities
pub mod display {
    /// Get refresh rate from monitor (platform-specific)
    #[cfg(target_os = "windows")]
    pub fn get_monitor_refresh_rate() -> Option<f64> {
        // TODO: Implement using EnumDisplaySettings
        None
    }

    #[cfg(target_os = "macos")]
    pub fn get_monitor_refresh_rate() -> Option<f64> {
        // TODO: Implement using CGDisplayModeGetRefreshRate
        None
    }

    #[cfg(target_os = "linux")]
    pub fn get_monitor_refresh_rate() -> Option<f64> {
        // TODO: Implement using XRandR or DRM
        None
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    pub fn get_monitor_refresh_rate() -> Option<f64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_default_refresh_rate() {
        let pacing = FramePacing::new();
        assert!((pacing.refresh_rate() - 60.0).abs() < 0.1);
    }

    #[test]
    fn test_set_refresh_rate() {
        let mut pacing = FramePacing::new();
        pacing.set_refresh_rate(144.0);
        assert!((pacing.refresh_rate() - 144.0).abs() < 0.1);

        let target = pacing.target_frame_duration();
        let expected = Duration::from_secs_f64(1.0 / 144.0);
        assert!((target.as_secs_f64() - expected.as_secs_f64()).abs() < 0.0001);
    }

    #[test]
    fn test_invalid_refresh_rate() {
        let mut pacing = FramePacing::new();
        pacing.set_refresh_rate(-10.0);
        assert!((pacing.refresh_rate() - 60.0).abs() < 0.1); // Should keep default

        pacing.set_refresh_rate(500.0);
        assert!((pacing.refresh_rate() - 60.0).abs() < 0.1); // Should keep default
    }

    #[test]
    fn test_should_generate_frame() {
        let mut pacing = FramePacing::new();
        pacing.set_refresh_rate(60.0);

        // Immediately after creation, should wait
        // (depends on timing, so we just test the API works)
        let _ = pacing.should_generate_frame();
    }

    #[test]
    fn test_frame_stats() {
        let mut pacing = FramePacing::new();

        // Simulate some frames
        for _ in 0..10 {
            pacing.begin_frame();
            pacing.on_frame_presented();
        }

        let stats = pacing.stats();
        assert_eq!(stats.total_frames, 10);
        assert!(stats.avg_frame_time_ms > 0.0);
    }

    #[test]
    fn test_vsync_disabled() {
        let config = FramePacingConfig {
            vsync_enabled: false,
            ..Default::default()
        };
        let pacing = FramePacing::with_config(config);

        // With vsync disabled, should always generate
        assert!(pacing.should_generate_frame());
        assert_eq!(pacing.time_until_next_frame(), Duration::ZERO);
    }

    #[test]
    fn test_reset_stats() {
        let mut pacing = FramePacing::new();

        for _ in 0..5 {
            pacing.on_frame_presented();
        }

        assert_eq!(pacing.stats().total_frames, 5);

        pacing.reset_stats();

        assert_eq!(pacing.stats().total_frames, 0);
        assert_eq!(pacing.stats().dropped_frames, 0);
    }
}
