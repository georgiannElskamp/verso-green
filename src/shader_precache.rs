//! Shader Precaching Control
//!
//! This module provides configurable shader precaching strategies
//! to optimize startup time vs runtime performance.

use webrender::ShaderPrecacheFlags;

/// Shader precaching strategy
///
/// Controls how WebRender handles shader compilation at startup.
/// Different strategies offer different tradeoffs between startup
/// time and runtime performance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShaderPrecacheStrategy {
    /// No precaching - compile shaders on demand
    ///
    /// **Pros**: Fastest startup time
    /// **Cons**: May cause stalls when new shaders are first used
    ///
    /// Best for: Development, quick testing
    None,

    /// Asynchronous compilation in background thread
    ///
    /// **Pros**: Fast startup, shaders ready when needed
    /// **Cons**: Brief stalls possible if shader needed before compiled
    ///
    /// Best for: Production use (default)
    #[default]
    Async,

    /// Full synchronous compilation at startup
    ///
    /// **Pros**: No runtime shader compilation stalls
    /// **Cons**: Slowest startup time (can be several seconds)
    ///
    /// Best for: Latency-critical applications, benchmarking
    Full,
}

impl ShaderPrecacheStrategy {
    /// Convert to WebRender's ShaderPrecacheFlags
    pub fn to_webrender_flags(self) -> ShaderPrecacheFlags {
        match self {
            ShaderPrecacheStrategy::None => ShaderPrecacheFlags::empty(),
            ShaderPrecacheStrategy::Async => ShaderPrecacheFlags::ASYNC_COMPILE,
            ShaderPrecacheStrategy::Full => ShaderPrecacheFlags::FULL_COMPILE,
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            ShaderPrecacheStrategy::None => "No precaching (fastest startup)",
            ShaderPrecacheStrategy::Async => "Async compilation (balanced)",
            ShaderPrecacheStrategy::Full => "Full precaching (no runtime stalls)",
        }
    }

    /// Parse from string (for configuration)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" | "disabled" | "off" => Some(ShaderPrecacheStrategy::None),
            "async" | "background" | "default" => Some(ShaderPrecacheStrategy::Async),
            "full" | "sync" | "all" => Some(ShaderPrecacheStrategy::Full),
            _ => None,
        }
    }
}

impl std::fmt::Display for ShaderPrecacheStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShaderPrecacheStrategy::None => write!(f, "none"),
            ShaderPrecacheStrategy::Async => write!(f, "async"),
            ShaderPrecacheStrategy::Full => write!(f, "full"),
        }
    }
}

/// Shader precaching configuration
#[derive(Clone, Debug)]
pub struct ShaderPrecacheConfig {
    /// Precaching strategy
    pub strategy: ShaderPrecacheStrategy,
    /// Path to custom shaders directory (if any)
    pub shaders_dir: Option<std::path::PathBuf>,
    /// Use optimized shaders (recommended for release)
    pub use_optimized_shaders: bool,
}

impl Default for ShaderPrecacheConfig {
    fn default() -> Self {
        Self {
            strategy: ShaderPrecacheStrategy::default(),
            shaders_dir: None,
            use_optimized_shaders: true,
        }
    }
}

impl ShaderPrecacheConfig {
    /// Create configuration for fastest startup
    pub fn fast_startup() -> Self {
        Self {
            strategy: ShaderPrecacheStrategy::None,
            shaders_dir: None,
            use_optimized_shaders: true,
        }
    }

    /// Create configuration for best runtime performance
    pub fn best_runtime() -> Self {
        Self {
            strategy: ShaderPrecacheStrategy::Full,
            shaders_dir: None,
            use_optimized_shaders: true,
        }
    }

    /// Create configuration for development
    pub fn development() -> Self {
        Self {
            strategy: ShaderPrecacheStrategy::None,
            shaders_dir: None,
            use_optimized_shaders: false, // Use debug shaders for better error messages
        }
    }
}

/// Shader compilation progress tracking
#[derive(Clone, Debug, Default)]
pub struct ShaderCompilationProgress {
    /// Total shaders to compile
    pub total: u32,
    /// Shaders compiled so far
    pub completed: u32,
    /// Compilation errors encountered
    pub errors: u32,
    /// Whether compilation is finished
    pub finished: bool,
}

impl ShaderCompilationProgress {
    /// Get completion percentage
    pub fn percentage(&self) -> f32 {
        if self.total == 0 {
            return if self.finished { 100.0 } else { 0.0 };
        }
        (self.completed as f32 / self.total as f32) * 100.0
    }

    /// Check if compilation is complete
    pub fn is_complete(&self) -> bool {
        self.finished || self.completed >= self.total
    }
}

/// Estimated startup time impact by strategy
pub mod startup_estimates {
    use super::ShaderPrecacheStrategy;
    use std::time::Duration;

    /// Get estimated additional startup time for a strategy
    ///
    /// These are rough estimates and vary significantly by hardware.
    /// GPU, driver version, and shader complexity all affect actual times.
    pub fn estimated_startup_impact(strategy: ShaderPrecacheStrategy) -> Duration {
        match strategy {
            ShaderPrecacheStrategy::None => Duration::from_millis(0),
            ShaderPrecacheStrategy::Async => Duration::from_millis(50), // Thread spawn overhead
            ShaderPrecacheStrategy::Full => Duration::from_millis(2000), // Full compilation
        }
    }

    /// Get estimated worst-case runtime stall for a strategy
    pub fn estimated_runtime_stall(strategy: ShaderPrecacheStrategy) -> Duration {
        match strategy {
            ShaderPrecacheStrategy::None => Duration::from_millis(100), // First use of any shader
            ShaderPrecacheStrategy::Async => Duration::from_millis(20), // Race condition
            ShaderPrecacheStrategy::Full => Duration::from_millis(0),   // All pre-compiled
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_strategy() {
        assert_eq!(ShaderPrecacheStrategy::default(), ShaderPrecacheStrategy::Async);
    }

    #[test]
    fn test_to_webrender_flags() {
        assert_eq!(
            ShaderPrecacheStrategy::None.to_webrender_flags(),
            ShaderPrecacheFlags::empty()
        );
        assert_eq!(
            ShaderPrecacheStrategy::Async.to_webrender_flags(),
            ShaderPrecacheFlags::ASYNC_COMPILE
        );
        assert_eq!(
            ShaderPrecacheStrategy::Full.to_webrender_flags(),
            ShaderPrecacheFlags::FULL_COMPILE
        );
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            ShaderPrecacheStrategy::from_str("none"),
            Some(ShaderPrecacheStrategy::None)
        );
        assert_eq!(
            ShaderPrecacheStrategy::from_str("ASYNC"),
            Some(ShaderPrecacheStrategy::Async)
        );
        assert_eq!(
            ShaderPrecacheStrategy::from_str("full"),
            Some(ShaderPrecacheStrategy::Full)
        );
        assert_eq!(ShaderPrecacheStrategy::from_str("invalid"), None);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", ShaderPrecacheStrategy::None), "none");
        assert_eq!(format!("{}", ShaderPrecacheStrategy::Async), "async");
        assert_eq!(format!("{}", ShaderPrecacheStrategy::Full), "full");
    }

    #[test]
    fn test_progress_percentage() {
        let mut progress = ShaderCompilationProgress {
            total: 100,
            completed: 50,
            errors: 0,
            finished: false,
        };
        assert!((progress.percentage() - 50.0).abs() < 0.01);

        progress.completed = 100;
        assert!((progress.percentage() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_progress_zero_total() {
        let progress = ShaderCompilationProgress {
            total: 0,
            completed: 0,
            errors: 0,
            finished: true,
        };
        assert!((progress.percentage() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_config_presets() {
        let fast = ShaderPrecacheConfig::fast_startup();
        assert_eq!(fast.strategy, ShaderPrecacheStrategy::None);

        let runtime = ShaderPrecacheConfig::best_runtime();
        assert_eq!(runtime.strategy, ShaderPrecacheStrategy::Full);

        let dev = ShaderPrecacheConfig::development();
        assert!(!dev.use_optimized_shaders);
    }
}
