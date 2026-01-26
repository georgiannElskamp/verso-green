//! Shader Precaching Configuration
//!
//! This module provides configurable shader precaching strategies
//! to optimize the tradeoff between startup time and runtime performance.

use webrender::ShaderPrecacheFlags;

/// Shader precaching strategy
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShaderPrecacheStrategy {
    /// No precaching - compile shaders on demand
    /// Fastest startup, but may cause stalls during first use of each shader
    None,

    /// Compile shaders asynchronously in background
    /// Good balance of startup time and runtime smoothness
    #[default]
    Async,

    /// Full synchronous compilation at startup
    /// Slowest startup, but no shader compilation stalls at runtime
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

    /// Get human-readable description of the strategy
    pub fn description(&self) -> &'static str {
        match self {
            ShaderPrecacheStrategy::None => {
                "No precaching - fastest startup, potential runtime stalls"
            }
            ShaderPrecacheStrategy::Async => {
                "Async compilation - balanced startup and runtime performance"
            }
            ShaderPrecacheStrategy::Full => {
                "Full precaching - slowest startup, smoothest runtime"
            }
        }
    }

    /// Parse from string (for config files/CLI)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" | "off" | "disabled" => Some(ShaderPrecacheStrategy::None),
            "async" | "background" => Some(ShaderPrecacheStrategy::Async),
            "full" | "sync" | "synchronous" => Some(ShaderPrecacheStrategy::Full),
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
    /// Use optimized (release) shaders even in debug builds
    pub use_optimized_shaders: bool,
    /// Custom shader directory override (None = use default)
    pub shader_dir: Option<std::path::PathBuf>,
}

impl Default for ShaderPrecacheConfig {
    fn default() -> Self {
        Self {
            strategy: ShaderPrecacheStrategy::default(),
            use_optimized_shaders: true,
            shader_dir: None,
        }
    }
}

impl ShaderPrecacheConfig {
    /// Create config for fastest startup (development/testing)
    pub fn fast_startup() -> Self {
        Self {
            strategy: ShaderPrecacheStrategy::None,
            use_optimized_shaders: false,
            shader_dir: None,
        }
    }

    /// Create config for smoothest runtime (production)
    pub fn smooth_runtime() -> Self {
        Self {
            strategy: ShaderPrecacheStrategy::Full,
            use_optimized_shaders: true,
            shader_dir: None,
        }
    }

    /// Create config for balanced performance (default)
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Convert to WebRender flags
    pub fn to_webrender_flags(&self) -> ShaderPrecacheFlags {
        self.strategy.to_webrender_flags()
    }
}

/// Shader compilation progress tracking
#[derive(Clone, Debug, Default)]
pub struct ShaderCompilationProgress {
    /// Total shaders to compile
    pub total: u32,
    /// Shaders compiled so far
    pub compiled: u32,
    /// Shaders that failed to compile
    pub failed: u32,
    /// Whether compilation is complete
    pub complete: bool,
}

impl ShaderCompilationProgress {
    /// Create new progress tracker
    pub fn new(total: u32) -> Self {
        Self {
            total,
            compiled: 0,
            failed: 0,
            complete: false,
        }
    }

    /// Get completion percentage
    pub fn percentage(&self) -> f32 {
        if self.total == 0 {
            100.0
        } else {
            ((self.compiled + self.failed) as f32 / self.total as f32) * 100.0
        }
    }

    /// Check if any shaders failed
    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }

    /// Mark a shader as compiled
    pub fn on_shader_compiled(&mut self) {
        self.compiled += 1;
        self.check_complete();
    }

    /// Mark a shader as failed
    pub fn on_shader_failed(&mut self) {
        self.failed += 1;
        log::warn!(
            "Shader compilation failed ({} failures so far)",
            self.failed
        );
        self.check_complete();
    }

    fn check_complete(&mut self) {
        if self.compiled + self.failed >= self.total {
            self.complete = true;
            if self.failed > 0 {
                log::warn!(
                    "Shader compilation complete: {} compiled, {} failed",
                    self.compiled,
                    self.failed
                );
            } else {
                log::info!("Shader compilation complete: {} shaders", self.compiled);
            }
        }
    }
}

/// Callback type for shader compilation progress
pub type ShaderProgressCallback = Box<dyn Fn(&ShaderCompilationProgress) + Send>;

/// Builder for shader precache configuration
pub struct ShaderPrecacheConfigBuilder {
    config: ShaderPrecacheConfig,
}

impl ShaderPrecacheConfigBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            config: ShaderPrecacheConfig::default(),
        }
    }

    /// Set the precaching strategy
    pub fn strategy(mut self, strategy: ShaderPrecacheStrategy) -> Self {
        self.config.strategy = strategy;
        self
    }

    /// Set whether to use optimized shaders
    pub fn use_optimized_shaders(mut self, use_optimized: bool) -> Self {
        self.config.use_optimized_shaders = use_optimized;
        self
    }

    /// Set custom shader directory
    pub fn shader_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.config.shader_dir = Some(dir.into());
        self
    }

    /// Build the configuration
    pub fn build(self) -> ShaderPrecacheConfig {
        self.config
    }
}

impl Default for ShaderPrecacheConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_strategy() {
        assert_eq!(
            ShaderPrecacheStrategy::default(),
            ShaderPrecacheStrategy::Async
        );
    }

    #[test]
    fn test_strategy_to_flags() {
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
    fn test_strategy_from_str() {
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
    fn test_progress_tracking() {
        let mut progress = ShaderCompilationProgress::new(10);
        assert_eq!(progress.percentage(), 0.0);
        assert!(!progress.complete);

        for _ in 0..8 {
            progress.on_shader_compiled();
        }
        assert_eq!(progress.percentage(), 80.0);

        progress.on_shader_failed();
        progress.on_shader_compiled();
        assert!(progress.complete);
        assert!(progress.has_failures());
    }

    #[test]
    fn test_config_builder() {
        let config = ShaderPrecacheConfigBuilder::new()
            .strategy(ShaderPrecacheStrategy::Full)
            .use_optimized_shaders(false)
            .build();

        assert_eq!(config.strategy, ShaderPrecacheStrategy::Full);
        assert!(!config.use_optimized_shaders);
    }

    #[test]
    fn test_preset_configs() {
        let fast = ShaderPrecacheConfig::fast_startup();
        assert_eq!(fast.strategy, ShaderPrecacheStrategy::None);

        let smooth = ShaderPrecacheConfig::smooth_runtime();
        assert_eq!(smooth.strategy, ShaderPrecacheStrategy::Full);

        let balanced = ShaderPrecacheConfig::balanced();
        assert_eq!(balanced.strategy, ShaderPrecacheStrategy::Async);
    }
}
