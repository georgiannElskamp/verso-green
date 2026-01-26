//! WebGL Support Infrastructure
//!
//! This module provides WebGL initialization and management,
//! gated behind the `webgl` feature flag.

/// WebGL configuration options
#[derive(Clone, Debug)]
pub struct WebGLConfig {
    /// Enable WebGL support
    pub enabled: bool,
    /// WebGL version to request (1 or 2)
    pub version: WebGLVersion,
    /// Allow software rendering fallback
    pub allow_software_fallback: bool,
    /// Maximum texture size (0 = driver default)
    pub max_texture_size: u32,
}

impl Default for WebGLConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            version: WebGLVersion::WebGL2,
            allow_software_fallback: true,
            max_texture_size: 0,
        }
    }
}

/// WebGL version selector
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebGLVersion {
    /// WebGL 1.0 (OpenGL ES 2.0)
    WebGL1,
    /// WebGL 2.0 (OpenGL ES 3.0)
    WebGL2,
}

impl Default for WebGLVersion {
    fn default() -> Self {
        Self::WebGL2
    }
}

/// Result of WebGL initialization attempt
#[derive(Debug)]
pub enum WebGLInitResult {
    /// WebGL initialized successfully
    Success {
        version: WebGLVersion,
        renderer: String,
    },
    /// WebGL initialization failed
    Failed {
        reason: String,
    },
    /// WebGL disabled by configuration
    Disabled,
}

/// Initialize WebGL support
/// 
/// This function attempts to initialize WebGL, handling various failure modes:
/// - GPU driver not available
/// - Context creation failure
/// - Feature disabled by configuration
///
/// # Arguments
/// * `config` - WebGL configuration options
///
/// # Returns
/// * `WebGLInitResult` indicating success or failure mode
#[cfg(feature = "webgl")]
pub fn init_webgl(config: &WebGLConfig) -> WebGLInitResult {
    use gleam::gl;

    if !config.enabled {
        log::info!("WebGL disabled by configuration");
        return WebGLInitResult::Disabled;
    }

    log::info!("Initializing WebGL support...");

    // Attempt WebGL 2 first if requested
    if config.version == WebGLVersion::WebGL2 {
        match try_init_webgl2() {
            Ok(renderer) => {
                log::info!("WebGL 2.0 initialized successfully on {}", renderer);
                return WebGLInitResult::Success {
                    version: WebGLVersion::WebGL2,
                    renderer,
                };
            }
            Err(e) => {
                log::warn!("WebGL 2.0 initialization failed: {}, trying WebGL 1.0", e);
            }
        }
    }

    // Fallback to WebGL 1
    match try_init_webgl1() {
        Ok(renderer) => {
            log::info!("WebGL 1.0 initialized successfully on {}", renderer);
            WebGLInitResult::Success {
                version: WebGLVersion::WebGL1,
                renderer,
            }
        }
        Err(e) => {
            log::error!("WebGL initialization failed completely: {}", e);
            WebGLInitResult::Failed {
                reason: e.to_string(),
            }
        }
    }
}

#[cfg(feature = "webgl")]
fn try_init_webgl2() -> Result<String, Box<dyn std::error::Error>> {
    // TODO: Implement actual WebGL 2.0 context creation
    // This requires integration with canvas_traits::webgl
    Err("WebGL 2.0 not yet implemented".into())
}

#[cfg(feature = "webgl")]
fn try_init_webgl1() -> Result<String, Box<dyn std::error::Error>> {
    // TODO: Implement actual WebGL 1.0 context creation
    Err("WebGL 1.0 not yet implemented".into())
}

/// Stub for when WebGL feature is disabled
#[cfg(not(feature = "webgl"))]
pub fn init_webgl(_config: &WebGLConfig) -> WebGLInitResult {
    log::info!("WebGL support not compiled in (feature disabled)");
    WebGLInitResult::Disabled
}

/// GPU blocklist entry
#[derive(Clone, Debug)]
pub struct GPUBlocklistEntry {
    /// Vendor ID pattern (regex)
    pub vendor_pattern: String,
    /// Device ID pattern (regex)
    pub device_pattern: String,
    /// Reason for blocking
    pub reason: String,
}

/// Check if current GPU is on the blocklist
pub fn is_gpu_blocked(_renderer: &str, _blocklist: &[GPUBlocklistEntry]) -> bool {
    // TODO: Implement GPU blocklist checking
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WebGLConfig::default();
        assert!(config.enabled);
        assert_eq!(config.version, WebGLVersion::WebGL2);
    }

    #[test]
    fn test_disabled_config() {
        let config = WebGLConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(!config.enabled);
    }
}
