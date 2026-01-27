//! WebGL Support Infrastructure
//!
//! This module provides WebGL initialization, context management, and compositor
//! integration, all gated behind the `webgl` feature flag.
//!
//! # Architecture
//!
//! WebGL in verso-green works through Servo's canvas implementation:
//!
//! 1. **Context Creation**: When JavaScript calls `canvas.getContext('webgl')`,
//!    Servo's script thread creates a WebGL context through the canvas backend.
//!
//! 2. **Rendering**: WebGL commands are executed on the WebGL context, which
//!    renders to a texture/framebuffer.
//!
//! 3. **Compositing**: The WebGL texture is exposed to WebRender as an external
//!    image, which composites it into the final scene.
//!
//! 4. **Resource Management**: Contexts are tracked per-pipeline and cleaned up
//!    when pipelines are removed.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "webgl")]
use std::rc::Rc;

#[cfg(feature = "webgl")]
use gleam::gl;

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
    /// Enable WebGL debug mode (slower but more error checking)
    pub debug_mode: bool,
    /// Antialias preference
    pub antialias: bool,
    /// Preserve drawing buffer (needed for some use cases)
    pub preserve_drawing_buffer: bool,
}

impl Default for WebGLConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            version: WebGLVersion::WebGL2,
            allow_software_fallback: true,
            max_texture_size: 0,
            debug_mode: false,
            antialias: true,
            preserve_drawing_buffer: false,
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
        /// Actual WebGL version initialized
        version: WebGLVersion,
        /// GPU renderer string
        renderer: String,
        /// GPU vendor string
        vendor: String,
        /// Maximum texture size supported
        max_texture_size: i32,
    },
    /// WebGL initialization failed
    Failed {
        /// Reason for failure
        reason: String,
    },
    /// WebGL disabled by configuration
    Disabled,
}

/// Unique identifier for a WebGL context
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WebGLContextId(u64);

impl WebGLContextId {
    /// Generate a new unique context ID
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value
    pub fn id(&self) -> u64 {
        self.0
    }
}

impl Default for WebGLContextId {
    fn default() -> Self {
        Self::new()
    }
}

/// State of a WebGL context
#[cfg(feature = "webgl")]
#[derive(Debug)]
pub struct WebGLContextState {
    /// Unique context identifier
    pub id: WebGLContextId,
    /// Width of the context
    pub width: u32,
    /// Height of the context
    pub height: u32,
    /// WebGL version
    pub version: WebGLVersion,
    /// Whether context is lost
    pub is_lost: bool,
    /// Associated image key for WebRender
    pub image_key: Option<webrender_api::ImageKey>,
}

#[cfg(feature = "webgl")]
impl WebGLContextState {
    /// Create a new context state
    pub fn new(id: WebGLContextId, width: u32, height: u32, version: WebGLVersion) -> Self {
        Self {
            id,
            width,
            height,
            version,
            is_lost: false,
            image_key: None,
        }
    }

    /// Resize the context
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Mark context as lost
    pub fn mark_lost(&mut self) {
        self.is_lost = true;
    }

    /// Mark context as restored
    pub fn mark_restored(&mut self) {
        self.is_lost = false;
    }
}

/// Manager for WebGL contexts
///
/// Tracks all WebGL contexts in the application, organized by pipeline ID.
/// This allows proper cleanup when pipelines are removed.
#[cfg(feature = "webgl")]
pub struct WebGLContextManager {
    /// Contexts indexed by context ID
    contexts: HashMap<WebGLContextId, WebGLContextState>,
    /// Mapping from pipeline to its contexts
    pipeline_contexts: HashMap<base::id::PipelineId, Vec<WebGLContextId>>,
    /// GL interface reference for context operations
    gl: Option<Rc<dyn gl::Gl>>,
    /// Configuration
    config: WebGLConfig,
}

#[cfg(feature = "webgl")]
impl WebGLContextManager {
    /// Create a new context manager
    pub fn new(config: WebGLConfig) -> Self {
        Self {
            contexts: HashMap::new(),
            pipeline_contexts: HashMap::new(),
            gl: None,
            config,
        }
    }

    /// Set the GL interface for this manager
    pub fn set_gl(&mut self, gl: Rc<dyn gl::Gl>) {
        self.gl = Some(gl);
    }

    /// Get the GL interface
    pub fn gl(&self) -> Option<&Rc<dyn gl::Gl>> {
        self.gl.as_ref()
    }

    /// Register a new WebGL context for a pipeline
    pub fn register_context(
        &mut self,
        pipeline_id: base::id::PipelineId,
        width: u32,
        height: u32,
        version: WebGLVersion,
    ) -> WebGLContextId {
        let id = WebGLContextId::new();
        let state = WebGLContextState::new(id, width, height, version);

        self.contexts.insert(id, state);
        self.pipeline_contexts
            .entry(pipeline_id)
            .or_default()
            .push(id);

        log::debug!(
            "Registered WebGL context {:?} for pipeline {:?}",
            id,
            pipeline_id
        );
        id
    }

    /// Get a context by ID
    pub fn get_context(&self, id: WebGLContextId) -> Option<&WebGLContextState> {
        self.contexts.get(&id)
    }

    /// Get a mutable context by ID
    pub fn get_context_mut(&mut self, id: WebGLContextId) -> Option<&mut WebGLContextState> {
        self.contexts.get_mut(&id)
    }

    /// Remove a specific context
    pub fn remove_context(&mut self, id: WebGLContextId) -> Option<WebGLContextState> {
        if let Some(state) = self.contexts.remove(&id) {
            // Remove from pipeline mapping
            for contexts in self.pipeline_contexts.values_mut() {
                contexts.retain(|&ctx_id| ctx_id != id);
            }
            log::debug!("Removed WebGL context {:?}", id);
            Some(state)
        } else {
            None
        }
    }

    /// Remove all contexts for a pipeline
    pub fn remove_pipeline_contexts(
        &mut self,
        pipeline_id: base::id::PipelineId,
    ) -> Vec<WebGLContextState> {
        let mut removed = Vec::new();

        if let Some(context_ids) = self.pipeline_contexts.remove(&pipeline_id) {
            for id in context_ids {
                if let Some(state) = self.contexts.remove(&id) {
                    removed.push(state);
                }
            }
        }

        if !removed.is_empty() {
            log::debug!(
                "Removed {} WebGL contexts for pipeline {:?}",
                removed.len(),
                pipeline_id
            );
        }
        removed
    }

    /// Get all context IDs for a pipeline
    pub fn get_pipeline_contexts(
        &self,
        pipeline_id: base::id::PipelineId,
    ) -> Option<&Vec<WebGLContextId>> {
        self.pipeline_contexts.get(&pipeline_id)
    }

    /// Get total number of active contexts
    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }

    /// Check if WebGL is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the configuration
    pub fn config(&self) -> &WebGLConfig {
        &self.config
    }
}

#[cfg(feature = "webgl")]
impl Default for WebGLContextManager {
    fn default() -> Self {
        Self::new(WebGLConfig::default())
    }
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
/// * `gl` - Reference to the GL interface
///
/// # Returns
/// * `WebGLInitResult` indicating success or failure mode
#[cfg(feature = "webgl")]
pub fn init_webgl(config: &WebGLConfig, gl: &dyn gl::Gl) -> WebGLInitResult {
    if !config.enabled {
        log::info!("WebGL disabled by configuration");
        return WebGLInitResult::Disabled;
    }

    log::info!("Initializing WebGL support...");

    // Query GL capabilities
    let renderer = gl.get_string(gl::RENDERER);
    let vendor = gl.get_string(gl::VENDOR);
    let version_string = gl.get_string(gl::VERSION);
    let max_texture_size = gl.get_integer_v(gl::MAX_TEXTURE_SIZE);

    log::info!("GL Renderer: {}", renderer);
    log::info!("GL Vendor: {}", vendor);
    log::info!("GL Version: {}", version_string);
    log::info!("Max Texture Size: {}", max_texture_size);

    // Check if we have sufficient OpenGL ES support
    let supports_gles3 = version_string.contains("OpenGL ES 3")
        || version_string.contains("4.")
        || version_string.contains("3.");

    let actual_version = if config.version == WebGLVersion::WebGL2 && supports_gles3 {
        WebGLVersion::WebGL2
    } else {
        WebGLVersion::WebGL1
    };

    if config.version == WebGLVersion::WebGL2 && actual_version == WebGLVersion::WebGL1 {
        log::warn!("WebGL 2.0 requested but not available, falling back to WebGL 1.0");
    }

    log::info!("WebGL {:?} initialized successfully", actual_version);

    WebGLInitResult::Success {
        version: actual_version,
        renderer,
        vendor,
        max_texture_size,
    }
}

/// Stub for when WebGL feature is disabled
#[cfg(not(feature = "webgl"))]
pub fn init_webgl(_config: &WebGLConfig) -> WebGLInitResult {
    log::info!("WebGL support not compiled in (feature disabled)");
    WebGLInitResult::Disabled
}

/// GPU blocklist entry for known problematic hardware
#[derive(Clone, Debug)]
pub struct GPUBlocklistEntry {
    /// Vendor pattern (substring match)
    pub vendor_pattern: String,
    /// Device/renderer pattern (substring match)
    pub device_pattern: String,
    /// Reason for blocking
    pub reason: String,
    /// Blocked WebGL versions (empty = all versions)
    pub blocked_versions: Vec<WebGLVersion>,
}

/// Default GPU blocklist for known problematic hardware
pub fn default_gpu_blocklist() -> Vec<GPUBlocklistEntry> {
    vec![
        // Software renderers with poor WebGL performance
        GPUBlocklistEntry {
            vendor_pattern: "Microsoft".to_string(),
            device_pattern: "Basic Render Driver".to_string(),
            reason: "Software renderer - poor WebGL performance".to_string(),
            blocked_versions: vec![WebGLVersion::WebGL2],
        },
        GPUBlocklistEntry {
            vendor_pattern: "VMware".to_string(),
            device_pattern: "SVGA3D".to_string(),
            reason: "Virtual GPU with limited WebGL 2 support".to_string(),
            blocked_versions: vec![WebGLVersion::WebGL2],
        },
    ]
}

/// Check if current GPU is on the blocklist
///
/// # Arguments
/// * `vendor` - GPU vendor string
/// * `renderer` - GPU renderer string
/// * `blocklist` - List of blocklist entries to check against
/// * `version` - WebGL version being requested
///
/// # Returns
/// * `Some(&GPUBlocklistEntry)` if GPU is blocked, `None` otherwise
pub fn is_gpu_blocked(
    vendor: &str,
    renderer: &str,
    blocklist: &[GPUBlocklistEntry],
    version: WebGLVersion,
) -> Option<&GPUBlocklistEntry> {
    for entry in blocklist {
        let vendor_match = vendor.contains(&entry.vendor_pattern);
        let device_match = renderer.contains(&entry.device_pattern);

        if vendor_match && device_match {
            // Check if this version is blocked
            if entry.blocked_versions.is_empty() || entry.blocked_versions.contains(&version) {
                return Some(entry);
            }
        }
    }
    None
}

/// WebGL capabilities query result
#[cfg(feature = "webgl")]
#[derive(Debug, Clone)]
pub struct WebGLCapabilities {
    /// Maximum texture size
    pub max_texture_size: i32,
    /// Maximum cube map texture size
    pub max_cube_map_texture_size: i32,
    /// Maximum renderbuffer size
    pub max_renderbuffer_size: i32,
    /// Maximum viewport dimensions
    pub max_viewport_dims: [i32; 2],
    /// Maximum vertex attributes
    pub max_vertex_attribs: i32,
    /// Maximum vertex uniform vectors
    pub max_vertex_uniform_vectors: i32,
    /// Maximum fragment uniform vectors
    pub max_fragment_uniform_vectors: i32,
    /// Maximum varying vectors
    pub max_varying_vectors: i32,
    /// Maximum texture image units
    pub max_texture_image_units: i32,
    /// Supported extensions
    pub extensions: Vec<String>,
}

#[cfg(feature = "webgl")]
impl WebGLCapabilities {
    /// Query capabilities from GL context
    pub fn query(gl: &dyn gl::Gl) -> Self {
        let max_viewport_dims = gl.get_integer_v(gl::MAX_VIEWPORT_DIMS);

        // Parse extensions
        let extensions_str = gl.get_string(gl::EXTENSIONS);
        let extensions: Vec<String> = extensions_str
            .split_whitespace()
            .map(String::from)
            .collect();

        Self {
            max_texture_size: gl.get_integer_v(gl::MAX_TEXTURE_SIZE),
            max_cube_map_texture_size: gl.get_integer_v(gl::MAX_CUBE_MAP_TEXTURE_SIZE),
            max_renderbuffer_size: gl.get_integer_v(gl::MAX_RENDERBUFFER_SIZE),
            max_viewport_dims: [max_viewport_dims, max_viewport_dims], // Simplified
            max_vertex_attribs: gl.get_integer_v(gl::MAX_VERTEX_ATTRIBS),
            max_vertex_uniform_vectors: gl.get_integer_v(gl::MAX_VERTEX_UNIFORM_VECTORS),
            max_fragment_uniform_vectors: gl.get_integer_v(gl::MAX_FRAGMENT_UNIFORM_VECTORS),
            max_varying_vectors: gl.get_integer_v(gl::MAX_VARYING_VECTORS),
            max_texture_image_units: gl.get_integer_v(gl::MAX_TEXTURE_IMAGE_UNITS),
            extensions,
        }
    }

    /// Check if an extension is supported
    pub fn has_extension(&self, name: &str) -> bool {
        self.extensions.iter().any(|ext| ext == name)
    }
}

/// Helper trait for integrating WebGL with the rendering context
#[cfg(feature = "webgl")]
pub trait WebGLRenderingSupport {
    /// Get the GL interface for WebGL context creation
    fn gl_for_webgl(&self) -> Rc<dyn gl::Gl>;

    /// Initialize WebGL with given configuration
    fn init_webgl_support(&self, config: &WebGLConfig) -> WebGLInitResult {
        init_webgl(config, self.gl_for_webgl().as_ref())
    }

    /// Query WebGL capabilities
    fn query_webgl_capabilities(&self) -> WebGLCapabilities {
        WebGLCapabilities::query(self.gl_for_webgl().as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WebGLConfig::default();
        assert!(config.enabled);
        assert_eq!(config.version, WebGLVersion::WebGL2);
        assert!(config.antialias);
    }

    #[test]
    fn test_disabled_config() {
        let config = WebGLConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(!config.enabled);
    }

    #[test]
    fn test_context_id_uniqueness() {
        let id1 = WebGLContextId::new();
        let id2 = WebGLContextId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_gpu_blocklist() {
        let blocklist = default_gpu_blocklist();

        // Test blocked GPU
        let result = is_gpu_blocked(
            "Microsoft Corporation",
            "Microsoft Basic Render Driver",
            &blocklist,
            WebGLVersion::WebGL2,
        );
        assert!(result.is_some());

        // Test same GPU but WebGL1 (should be allowed)
        let result = is_gpu_blocked(
            "Microsoft Corporation",
            "Microsoft Basic Render Driver",
            &blocklist,
            WebGLVersion::WebGL1,
        );
        assert!(result.is_none());

        // Test allowed GPU
        let result = is_gpu_blocked(
            "NVIDIA Corporation",
            "GeForce RTX 3080",
            &blocklist,
            WebGLVersion::WebGL2,
        );
        assert!(result.is_none());
    }

    #[cfg(feature = "webgl")]
    mod webgl_tests {
        use super::*;

        #[test]
        fn test_context_state() {
            let id = WebGLContextId::new();
            let mut state = WebGLContextState::new(id, 800, 600, WebGLVersion::WebGL2);

            assert_eq!(state.width, 800);
            assert_eq!(state.height, 600);
            assert!(!state.is_lost);

            state.resize(1024, 768);
            assert_eq!(state.width, 1024);
            assert_eq!(state.height, 768);

            state.mark_lost();
            assert!(state.is_lost);

            state.mark_restored();
            assert!(!state.is_lost);
        }

        #[test]
        fn test_context_manager_basic() {
            let manager = WebGLContextManager::new(WebGLConfig::default());
            assert!(manager.is_enabled());
            assert_eq!(manager.context_count(), 0);
        }
    }
}
