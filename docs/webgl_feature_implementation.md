# WebGL Feature Implementation Guide

## Overview

This document tracks the implementation of WebGL support in verso-green through the `webgl` feature flag.

## Feature Flag Status

✅ **A1.1 Complete**: The `webgl` feature flag has been added to `Cargo.toml` with proper dependency propagation.

✅ **A1.2 Complete**: Feature guards verified - WebGL implementation is in Servo dependencies.

✅ **A1.3 Complete**: WebGL context creation and compositor integration implemented.

## Implementation Architecture

### How WebGL Works in verso-green

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   JavaScript    │     │  Servo Canvas   │     │    WebRender    │
│                 │     │                 │     │                 │
│ canvas.getContext────▶│ WebGL Context   ────▶│ External Image  │
│   ('webgl')     │     │   Creation      │     │   Compositing   │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                              │                         │
                              ▼                         ▼
                    ┌─────────────────┐     ┌─────────────────┐
                    │ WebGLContext    │     │ RenderingContext│
                    │    Manager      │◀────│   (verso-green) │
                    │ (verso-green)   │     │   GL Interface  │
                    └─────────────────┘     └─────────────────┘
```

### Key Components

#### 1. `src/webgl_support.rs`

The main WebGL support module containing:

| Component | Description |
|-----------|-------------|
| `WebGLConfig` | Configuration options (version, antialias, etc.) |
| `WebGLContextId` | Unique identifier for contexts |
| `WebGLContextState` | State tracking per context |
| `WebGLContextManager` | Context lifecycle management |
| `WebGLCapabilities` | GPU capability queries |
| `init_webgl()` | Initialization with version detection |
| `is_gpu_blocked()` | GPU blocklist checking |

#### 2. `src/rendering.rs`

Integration with the rendering context:

```rust
// WebGL support trait implementation
#[cfg(feature = "webgl")]
impl WebGLRenderingSupport for RenderingContext {
    fn gl_for_webgl(&self) -> Rc<dyn gl::Gl> {
        self.gl_rc()
    }
}
```

#### 3. Servo Dependencies

Most WebGL implementation lives in Servo crates:

| Crate | Feature | Purpose |
|-------|---------|----------|
| `webgpu` | `webgl` | WebGL context and rendering |
| `webgpu_traits` | `webgl` | WebGL trait definitions |
| `script` | `webgl` | WebGL DOM APIs |
| `canvas` | `webgl` | Canvas WebGL backend |

## Usage

### Building with WebGL

```bash
# Build without WebGL (default)
cargo build

# Build with WebGL support
cargo build --features webgl
```

### Runtime Initialization

```rust
use verso::webgl_support::{WebGLConfig, init_webgl, WebGLRenderingSupport};

// Initialize WebGL through the rendering context
let config = WebGLConfig::default();
let result = rendering_context.init_webgl_support(&config);

match result {
    WebGLInitResult::Success { version, renderer, .. } => {
        println!("WebGL {:?} on {}", version, renderer);
    }
    WebGLInitResult::Failed { reason } => {
        eprintln!("WebGL init failed: {}", reason);
    }
    WebGLInitResult::Disabled => {
        println!("WebGL disabled by config");
    }
}
```

### Context Management

```rust
use verso::webgl_support::{WebGLContextManager, WebGLVersion};

let mut manager = WebGLContextManager::new(config);
manager.set_gl(rendering_context.gl_rc());

// Register a context for a pipeline
let ctx_id = manager.register_context(
    pipeline_id,
    800,  // width
    600,  // height
    WebGLVersion::WebGL2
);

// Clean up when pipeline is removed
let removed = manager.remove_pipeline_contexts(pipeline_id);
```

## GPU Blocklist

Known problematic GPUs are blocked from certain WebGL versions:

| Vendor | Device | Blocked Versions | Reason |
|--------|--------|------------------|--------|
| Microsoft | Basic Render Driver | WebGL 2 | Software renderer |
| VMware | SVGA3D | WebGL 2 | Limited WebGL 2 support |

## Testing Strategy

### Build Tests

```bash
# Test compilation without webgl
cargo check
cargo check --features webgl

# Run tests
cargo test
cargo test --features webgl
```

### WebGL Feature Tests

The `webgl_support` module includes unit tests:

```bash
cargo test --features webgl webgl_tests
```

### Manual Testing

To verify WebGL works:

1. Build with `--features webgl`
2. Navigate to a WebGL test page (e.g., https://get.webgl.org/)
3. Verify the spinning cube renders correctly

## Current Status

| Task | Status | Notes |
|------|--------|-------|
| A1.1: Add feature flag | ✅ Complete | In Cargo.toml |
| A1.2: Feature guards | ✅ Complete | Verified in Servo deps |
| A1.3: Context creation | ✅ Complete | WebGLContextManager implemented |
| A1.4: Testing | ⏳ Pending | Next task |

## Implementation Details

### Context Lifecycle

1. **Creation**: When `canvas.getContext('webgl')` is called:
   - Servo's script creates a WebGL context via canvas backend
   - Context is registered with `WebGLContextManager`
   - Image key generated for WebRender compositing

2. **Rendering**: Each frame:
   - WebGL commands execute on the context
   - Rendered to framebuffer/texture
   - WebRender composites as external image

3. **Resize**: When canvas dimensions change:
   - Context state updated via `resize()`
   - WebRender texture descriptor updated

4. **Loss/Restoration**: On GPU reset:
   - `mark_lost()` called on context state
   - `webglcontextlost` event fired
   - On restoration: `mark_restored()` and `webglcontextrestored`

5. **Cleanup**: When pipeline is removed:
   - `remove_pipeline_contexts()` cleans up all contexts
   - WebRender image keys deleted
   - GL resources freed

### Feature-Gated Code

All WebGL code is conditionally compiled:

```rust
#[cfg(feature = "webgl")]
pub struct WebGLContextState { ... }

#[cfg(feature = "webgl")]
impl WebGLRenderingSupport for RenderingContext { ... }

#[cfg(not(feature = "webgl"))]
pub fn init_webgl(_: &WebGLConfig) -> WebGLInitResult {
    WebGLInitResult::Disabled
}
```

## References

- [Servo Canvas Documentation](https://book.servo.org/design-documentation/canvas.html)
- [WebGL Specification](https://www.khronos.org/webgl/)
- [surfman - WebGL Context Manager](https://github.com/servo/surfman)
- [WebRender External Images](https://github.com/nicokoch/webrender/wiki/External-Images)
