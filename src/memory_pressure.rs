//! Memory Pressure Handling
//!
//! This module provides memory pressure detection and response mechanisms
//! to prevent OOM conditions and maintain browser responsiveness.

use std::time::{Duration, Instant};

/// Memory pressure severity levels
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryPressureLevel {
    /// Normal operation, no memory constraints
    Normal,
    /// Warning level (>70% memory usage) - start evicting caches
    Warning,
    /// Critical level (>90% memory usage) - aggressive eviction + suspend pipelines
    Critical,
}

impl Default for MemoryPressureLevel {
    fn default() -> Self {
        Self::Normal
    }
}

/// Configuration for memory pressure handling
#[derive(Clone, Debug)]
pub struct MemoryPressureConfig {
    /// Threshold percentage for Warning level (default: 70%)
    pub warning_threshold: f64,
    /// Threshold percentage for Critical level (default: 90%)
    pub critical_threshold: f64,
    /// Minimum interval between pressure checks
    pub check_interval: Duration,
    /// Cache size reduction factor for Warning level
    pub warning_cache_factor: f32,
    /// Cache size reduction factor for Critical level
    pub critical_cache_factor: f32,
}

impl Default for MemoryPressureConfig {
    fn default() -> Self {
        Self {
            warning_threshold: 70.0,
            critical_threshold: 90.0,
            check_interval: Duration::from_secs(5),
            warning_cache_factor: 0.5,
            critical_cache_factor: 0.25,
        }
    }
}

/// Memory pressure monitor state
pub struct MemoryPressureMonitor {
    config: MemoryPressureConfig,
    last_check: Instant,
    current_level: MemoryPressureLevel,
}

impl MemoryPressureMonitor {
    /// Create a new memory pressure monitor
    pub fn new(config: MemoryPressureConfig) -> Self {
        Self {
            config,
            last_check: Instant::now(),
            current_level: MemoryPressureLevel::Normal,
        }
    }

    /// Check if it's time to evaluate memory pressure
    pub fn should_check(&self) -> bool {
        self.last_check.elapsed() >= self.config.check_interval
    }

    /// Evaluate current memory pressure level
    pub fn check(&mut self) -> MemoryPressureLevel {
        if !self.should_check() {
            return self.current_level;
        }

        self.last_check = Instant::now();
        let usage = self.get_memory_usage_percent();

        self.current_level = if usage > self.config.critical_threshold {
            log::warn!("Critical memory pressure: {:.1}% usage", usage);
            MemoryPressureLevel::Critical
        } else if usage > self.config.warning_threshold {
            log::info!("Warning memory pressure: {:.1}% usage", usage);
            MemoryPressureLevel::Warning
        } else {
            MemoryPressureLevel::Normal
        };

        self.current_level
    }

    /// Get current pressure level without re-checking
    pub fn current_level(&self) -> MemoryPressureLevel {
        self.current_level
    }

    /// Get cache reduction factor for current level
    pub fn cache_reduction_factor(&self) -> f32 {
        match self.current_level {
            MemoryPressureLevel::Normal => 1.0,
            MemoryPressureLevel::Warning => self.config.warning_cache_factor,
            MemoryPressureLevel::Critical => self.config.critical_cache_factor,
        }
    }

    /// Get memory usage percentage (platform-specific)
    fn get_memory_usage_percent(&self) -> f64 {
        #[cfg(target_os = "linux")]
        {
            self.get_linux_memory_usage()
        }

        #[cfg(target_os = "macos")]
        {
            self.get_macos_memory_usage()
        }

        #[cfg(target_os = "windows")]
        {
            self.get_windows_memory_usage()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            // Unknown platform, assume no pressure
            0.0
        }
    }

    #[cfg(target_os = "linux")]
    fn get_linux_memory_usage(&self) -> f64 {
        use std::fs;

        if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            let mut total = 0u64;
            let mut available = 0u64;

            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    total = parse_meminfo_kb(line);
                } else if line.starts_with("MemAvailable:") {
                    available = parse_meminfo_kb(line);
                }
            }

            if total > 0 {
                return ((total - available) as f64 / total as f64) * 100.0;
            }
        }

        0.0 // Fallback: assume no pressure
    }

    #[cfg(target_os = "macos")]
    fn get_macos_memory_usage(&self) -> f64 {
        // TODO: Implement using mach APIs or vm_statistics
        // For now, return 0 (no pressure detected)
        0.0
    }

    #[cfg(target_os = "windows")]
    fn get_windows_memory_usage(&self) -> f64 {
        // TODO: Implement using GlobalMemoryStatusEx
        // For now, return 0 (no pressure detected)
        0.0
    }
}

impl Default for MemoryPressureMonitor {
    fn default() -> Self {
        Self::new(MemoryPressureConfig::default())
    }
}

/// Parse a line from /proc/meminfo and extract the KB value
#[cfg(target_os = "linux")]
fn parse_meminfo_kb(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Actions to take in response to memory pressure
pub trait MemoryPressureHandler {
    /// Handle memory pressure event
    fn handle_memory_pressure(&mut self, level: MemoryPressureLevel);

    /// Evict cached images
    fn evict_image_caches(&mut self);

    /// Reduce WebRender cache sizes
    fn reduce_webrender_cache_size(&mut self, factor: f32);

    /// Suspend background pipelines
    fn suspend_background_pipelines(&mut self);

    /// Resume suspended pipelines when pressure subsides
    fn resume_suspended_pipelines(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MemoryPressureConfig::default();
        assert_eq!(config.warning_threshold, 70.0);
        assert_eq!(config.critical_threshold, 90.0);
    }

    #[test]
    fn test_pressure_level_default() {
        let monitor = MemoryPressureMonitor::default();
        assert_eq!(monitor.current_level(), MemoryPressureLevel::Normal);
    }

    #[test]
    fn test_cache_reduction_factors() {
        let mut monitor = MemoryPressureMonitor::default();
        
        monitor.current_level = MemoryPressureLevel::Normal;
        assert_eq!(monitor.cache_reduction_factor(), 1.0);
        
        monitor.current_level = MemoryPressureLevel::Warning;
        assert_eq!(monitor.cache_reduction_factor(), 0.5);
        
        monitor.current_level = MemoryPressureLevel::Critical;
        assert_eq!(monitor.cache_reduction_factor(), 0.25);
    }
}
