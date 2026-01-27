//! WebGL Test Suite
//!
//! This test suite verifies WebGL functionality in verso-green.
//! Tests are feature-gated and only run when the `webgl` feature is enabled.
//!
//! Run with: `cargo test --features webgl --test webgl_tests`

#![cfg(feature = "webgl")]

use verso::webgl_support::*;
use verso::verso_test;
use winit::event_loop::EventLoopWindowTarget;

// ============================================================================
// Configuration Tests
// ============================================================================

/// Test default WebGL configuration values
fn test_webgl_config_defaults(_elwt: &EventLoopWindowTarget<()>) {
    let config = WebGLConfig::default();
    
    assert!(config.enabled, "WebGL should be enabled by default");
    assert_eq!(config.version, WebGLVersion::WebGL2, "Should default to WebGL 2");
    assert!(config.allow_software_fallback, "Software fallback should be allowed");
    assert_eq!(config.max_texture_size, 0, "Max texture size should be 0 (driver default)");
    assert!(!config.debug_mode, "Debug mode should be off by default");
    assert!(config.antialias, "Antialias should be enabled by default");
    assert!(!config.preserve_drawing_buffer, "Preserve drawing buffer should be off");
}

/// Test custom WebGL configuration
fn test_webgl_config_custom(_elwt: &EventLoopWindowTarget<()>) {
    let config = WebGLConfig {
        enabled: false,
        version: WebGLVersion::WebGL1,
        allow_software_fallback: false,
        max_texture_size: 4096,
        debug_mode: true,
        antialias: false,
        preserve_drawing_buffer: true,
    };
    
    assert!(!config.enabled);
    assert_eq!(config.version, WebGLVersion::WebGL1);
    assert!(!config.allow_software_fallback);
    assert_eq!(config.max_texture_size, 4096);
    assert!(config.debug_mode);
    assert!(!config.antialias);
    assert!(config.preserve_drawing_buffer);
}

// ============================================================================
// Context ID Tests
// ============================================================================

/// Test that context IDs are unique
fn test_context_id_uniqueness(_elwt: &EventLoopWindowTarget<()>) {
    let id1 = WebGLContextId::new();
    let id2 = WebGLContextId::new();
    let id3 = WebGLContextId::new();
    
    assert_ne!(id1, id2, "Context IDs should be unique");
    assert_ne!(id2, id3, "Context IDs should be unique");
    assert_ne!(id1, id3, "Context IDs should be unique");
}

/// Test that context IDs are monotonically increasing
fn test_context_id_ordering(_elwt: &EventLoopWindowTarget<()>) {
    let id1 = WebGLContextId::new();
    let id2 = WebGLContextId::new();
    
    assert!(id2.id() > id1.id(), "Later IDs should have higher values");
}

/// Test default context ID creation
fn test_context_id_default(_elwt: &EventLoopWindowTarget<()>) {
    let id1 = WebGLContextId::default();
    let id2 = WebGLContextId::default();
    
    // Default should also create unique IDs
    assert_ne!(id1, id2);
}

// ============================================================================
// Context State Tests
// ============================================================================

/// Test context state creation
fn test_context_state_creation(_elwt: &EventLoopWindowTarget<()>) {
    let id = WebGLContextId::new();
    let state = WebGLContextState::new(id, 800, 600, WebGLVersion::WebGL2);
    
    assert_eq!(state.id, id);
    assert_eq!(state.width, 800);
    assert_eq!(state.height, 600);
    assert_eq!(state.version, WebGLVersion::WebGL2);
    assert!(!state.is_lost, "New context should not be lost");
    assert!(state.image_key.is_none(), "New context should have no image key");
}

/// Test context state resize
fn test_context_state_resize(_elwt: &EventLoopWindowTarget<()>) {
    let id = WebGLContextId::new();
    let mut state = WebGLContextState::new(id, 800, 600, WebGLVersion::WebGL2);
    
    state.resize(1920, 1080);
    
    assert_eq!(state.width, 1920);
    assert_eq!(state.height, 1080);
}

/// Test context lost/restored state transitions
fn test_context_state_lost_restored(_elwt: &EventLoopWindowTarget<()>) {
    let id = WebGLContextId::new();
    let mut state = WebGLContextState::new(id, 800, 600, WebGLVersion::WebGL1);
    
    assert!(!state.is_lost, "Initially not lost");
    
    state.mark_lost();
    assert!(state.is_lost, "Should be lost after mark_lost()");
    
    state.mark_restored();
    assert!(!state.is_lost, "Should be restored after mark_restored()");
}

// ============================================================================
// Context Manager Tests
// ============================================================================

/// Test context manager creation
fn test_context_manager_creation(_elwt: &EventLoopWindowTarget<()>) {
    let manager = WebGLContextManager::new(WebGLConfig::default());
    
    assert!(manager.is_enabled(), "Manager should be enabled with default config");
    assert_eq!(manager.context_count(), 0, "New manager should have no contexts");
}

/// Test context manager with disabled config
fn test_context_manager_disabled(_elwt: &EventLoopWindowTarget<()>) {
    let config = WebGLConfig {
        enabled: false,
        ..Default::default()
    };
    let manager = WebGLContextManager::new(config);
    
    assert!(!manager.is_enabled(), "Manager should be disabled");
}

/// Test registering contexts with the manager
fn test_context_manager_register(_elwt: &EventLoopWindowTarget<()>) {
    let mut manager = WebGLContextManager::new(WebGLConfig::default());
    let pipeline_id = base::id::PipelineId::new(1, 1);
    
    let ctx_id = manager.register_context(pipeline_id, 640, 480, WebGLVersion::WebGL2);
    
    assert_eq!(manager.context_count(), 1);
    assert!(manager.get_context(ctx_id).is_some());
    
    let state = manager.get_context(ctx_id).unwrap();
    assert_eq!(state.width, 640);
    assert_eq!(state.height, 480);
    assert_eq!(state.version, WebGLVersion::WebGL2);
}

/// Test registering multiple contexts for one pipeline
fn test_context_manager_multiple_contexts(_elwt: &EventLoopWindowTarget<()>) {
    let mut manager = WebGLContextManager::new(WebGLConfig::default());
    let pipeline_id = base::id::PipelineId::new(1, 1);
    
    let ctx1 = manager.register_context(pipeline_id, 640, 480, WebGLVersion::WebGL1);
    let ctx2 = manager.register_context(pipeline_id, 800, 600, WebGLVersion::WebGL2);
    let ctx3 = manager.register_context(pipeline_id, 1024, 768, WebGLVersion::WebGL2);
    
    assert_eq!(manager.context_count(), 3);
    assert_ne!(ctx1, ctx2);
    assert_ne!(ctx2, ctx3);
    
    let pipeline_contexts = manager.get_pipeline_contexts(pipeline_id).unwrap();
    assert_eq!(pipeline_contexts.len(), 3);
}

/// Test registering contexts for different pipelines
fn test_context_manager_multiple_pipelines(_elwt: &EventLoopWindowTarget<()>) {
    let mut manager = WebGLContextManager::new(WebGLConfig::default());
    let pipeline1 = base::id::PipelineId::new(1, 1);
    let pipeline2 = base::id::PipelineId::new(1, 2);
    
    manager.register_context(pipeline1, 640, 480, WebGLVersion::WebGL2);
    manager.register_context(pipeline1, 800, 600, WebGLVersion::WebGL2);
    manager.register_context(pipeline2, 1024, 768, WebGLVersion::WebGL1);
    
    assert_eq!(manager.context_count(), 3);
    assert_eq!(manager.get_pipeline_contexts(pipeline1).unwrap().len(), 2);
    assert_eq!(manager.get_pipeline_contexts(pipeline2).unwrap().len(), 1);
}

/// Test removing a single context
fn test_context_manager_remove_context(_elwt: &EventLoopWindowTarget<()>) {
    let mut manager = WebGLContextManager::new(WebGLConfig::default());
    let pipeline_id = base::id::PipelineId::new(1, 1);
    
    let ctx1 = manager.register_context(pipeline_id, 640, 480, WebGLVersion::WebGL2);
    let ctx2 = manager.register_context(pipeline_id, 800, 600, WebGLVersion::WebGL2);
    
    assert_eq!(manager.context_count(), 2);
    
    let removed = manager.remove_context(ctx1);
    assert!(removed.is_some());
    assert_eq!(manager.context_count(), 1);
    assert!(manager.get_context(ctx1).is_none());
    assert!(manager.get_context(ctx2).is_some());
}

/// Test removing all contexts for a pipeline
fn test_context_manager_remove_pipeline(_elwt: &EventLoopWindowTarget<()>) {
    let mut manager = WebGLContextManager::new(WebGLConfig::default());
    let pipeline1 = base::id::PipelineId::new(1, 1);
    let pipeline2 = base::id::PipelineId::new(1, 2);
    
    manager.register_context(pipeline1, 640, 480, WebGLVersion::WebGL2);
    manager.register_context(pipeline1, 800, 600, WebGLVersion::WebGL2);
    manager.register_context(pipeline2, 1024, 768, WebGLVersion::WebGL1);
    
    let removed = manager.remove_pipeline_contexts(pipeline1);
    
    assert_eq!(removed.len(), 2);
    assert_eq!(manager.context_count(), 1);
    assert!(manager.get_pipeline_contexts(pipeline1).is_none());
    assert!(manager.get_pipeline_contexts(pipeline2).is_some());
}

/// Test modifying context through manager
fn test_context_manager_modify_context(_elwt: &EventLoopWindowTarget<()>) {
    let mut manager = WebGLContextManager::new(WebGLConfig::default());
    let pipeline_id = base::id::PipelineId::new(1, 1);
    
    let ctx_id = manager.register_context(pipeline_id, 640, 480, WebGLVersion::WebGL2);
    
    // Modify through mutable reference
    if let Some(state) = manager.get_context_mut(ctx_id) {
        state.resize(1280, 720);
        state.mark_lost();
    }
    
    let state = manager.get_context(ctx_id).unwrap();
    assert_eq!(state.width, 1280);
    assert_eq!(state.height, 720);
    assert!(state.is_lost);
}

// ============================================================================
// GPU Blocklist Tests
// ============================================================================

/// Test default GPU blocklist
fn test_default_gpu_blocklist(_elwt: &EventLoopWindowTarget<()>) {
    let blocklist = default_gpu_blocklist();
    
    assert!(!blocklist.is_empty(), "Default blocklist should have entries");
}

/// Test GPU blocklist matching - blocked GPU
fn test_gpu_blocklist_blocked(_elwt: &EventLoopWindowTarget<()>) {
    let blocklist = default_gpu_blocklist();
    
    // Microsoft Basic Render Driver should be blocked for WebGL2
    let result = is_gpu_blocked(
        "Microsoft Corporation",
        "Microsoft Basic Render Driver",
        &blocklist,
        WebGLVersion::WebGL2,
    );
    
    assert!(result.is_some(), "Software renderer should be blocked for WebGL2");
    let entry = result.unwrap();
    assert!(entry.reason.contains("Software") || entry.reason.contains("renderer"));
}

/// Test GPU blocklist matching - allowed GPU
fn test_gpu_blocklist_allowed(_elwt: &EventLoopWindowTarget<()>) {
    let blocklist = default_gpu_blocklist();
    
    // NVIDIA should not be blocked
    let result = is_gpu_blocked(
        "NVIDIA Corporation",
        "NVIDIA GeForce RTX 4090",
        &blocklist,
        WebGLVersion::WebGL2,
    );
    
    assert!(result.is_none(), "Modern NVIDIA GPU should not be blocked");
}

/// Test GPU blocklist - WebGL1 fallback allowed
fn test_gpu_blocklist_version_specific(_elwt: &EventLoopWindowTarget<()>) {
    let blocklist = default_gpu_blocklist();
    
    // Microsoft Basic Render Driver should be allowed for WebGL1
    let result = is_gpu_blocked(
        "Microsoft Corporation",
        "Microsoft Basic Render Driver",
        &blocklist,
        WebGLVersion::WebGL1,
    );
    
    assert!(result.is_none(), "Software renderer should be allowed for WebGL1");
}

/// Test custom blocklist entry
fn test_gpu_blocklist_custom(_elwt: &EventLoopWindowTarget<()>) {
    let custom_blocklist = vec![
        GPUBlocklistEntry {
            vendor_pattern: "TestVendor".to_string(),
            device_pattern: "BadGPU".to_string(),
            reason: "Known to crash".to_string(),
            blocked_versions: vec![], // Block all versions
        },
    ];
    
    // Should match
    let result = is_gpu_blocked(
        "TestVendor Inc.",
        "BadGPU 9000",
        &custom_blocklist,
        WebGLVersion::WebGL1,
    );
    assert!(result.is_some());
    
    // Should not match
    let result = is_gpu_blocked(
        "OtherVendor",
        "GoodGPU",
        &custom_blocklist,
        WebGLVersion::WebGL1,
    );
    assert!(result.is_none());
}

// ============================================================================
// WebGL Version Tests
// ============================================================================

/// Test WebGL version enum
fn test_webgl_version_values(_elwt: &EventLoopWindowTarget<()>) {
    assert_ne!(WebGLVersion::WebGL1, WebGLVersion::WebGL2);
    assert_eq!(WebGLVersion::default(), WebGLVersion::WebGL2);
}

// ============================================================================
// Init Result Tests
// ============================================================================

/// Test disabled init result
fn test_webgl_init_disabled(_elwt: &EventLoopWindowTarget<()>) {
    let config = WebGLConfig {
        enabled: false,
        ..Default::default()
    };
    
    // When WebGL is disabled, init should return Disabled
    // Note: We can't call init_webgl without a GL context in this test,
    // but we can verify the config is properly set
    assert!(!config.enabled);
}

// ============================================================================
// Test Runner
// ============================================================================

verso_test!(
    // Configuration tests
    test_webgl_config_defaults,
    test_webgl_config_custom,
    
    // Context ID tests
    test_context_id_uniqueness,
    test_context_id_ordering,
    test_context_id_default,
    
    // Context state tests
    test_context_state_creation,
    test_context_state_resize,
    test_context_state_lost_restored,
    
    // Context manager tests
    test_context_manager_creation,
    test_context_manager_disabled,
    test_context_manager_register,
    test_context_manager_multiple_contexts,
    test_context_manager_multiple_pipelines,
    test_context_manager_remove_context,
    test_context_manager_remove_pipeline,
    test_context_manager_modify_context,
    
    // GPU blocklist tests
    test_default_gpu_blocklist,
    test_gpu_blocklist_blocked,
    test_gpu_blocklist_allowed,
    test_gpu_blocklist_version_specific,
    test_gpu_blocklist_custom,
    
    // Version tests
    test_webgl_version_values,
    
    // Init tests
    test_webgl_init_disabled
);
