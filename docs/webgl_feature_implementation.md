# WebGL Feature Implementation Guide

## Overview

This document tracks the implementation of WebGL support in verso-green through the `webgl` feature flag.

## Feature Flag Status

✅ **A1.1 Complete**: The `webgl` feature flag has been added to `Cargo.toml` with proper dependency propagation.

## Implementation Strategy

Since verso-green uses Servo as a web engine, most WebGL implementation exists within the Servo dependencies:
- `webgpu` crate - Contains WebGL context and rendering implementation
- `webgpu_traits` crate - Contains WebGL trait definitions
- `script` crate (with `webgl` feature) - Contains WebGL DOM APIs
- `canvas` crate (with `webgl` feature) - Contains canvas WebGL backend

### Local Code Areas to Check

The following areas in verso-green may need feature guards:

#### 1. Compositor (`src/compositor.rs`)

**Status**: ✅ No WebGL-specific code found that needs feature guards

The compositor handles all canvas types generically through:
- `CompositorMsg::GenerateImageKey` - Works for all image types including WebGL textures
- `CompositorMsg::UpdateImages` - Generic image updates
- External image handling through WebRender's `ExternalImageData`

**Action**: No changes needed. WebGL textures are treated as external images by WebRender.

#### 2. Message Definitions (`compositing_traits`)

**Status**: 🔍 **Needs Investigation**

Check if `CompositorMsg` enum in Servo's `compositing_traits` crate contains WebGL-specific variants that should be feature-gated.

**Potential WebGL messages**:
```rust
#[cfg(feature = "webgl")]
WebGLContextCreated(PipelineId, WebGLContextId, ImageKey),

#[cfg(feature = "webgl")]
WebGLContextResized(WebGLContextId, Size2D<u32, DevicePixel>),

#[cfg(feature = "webgl")]
WebGLContextLost(WebGLContextId),
```

**Action**: These messages are defined in Servo's crate and already feature-gated in upstream.

#### 3. Rendering Context (`src/rendering.rs`)

**Status**: 🔍 **Needs Investigation**

Check if GL context sharing for WebGL needs feature guards.

```rust
#[cfg(feature = "webgl")]
impl RenderingContext {
    pub fn gl_context_for_webgl(&self) -> Option<&Rc<dyn gl::Gl>> {
        // Return GL context for WebGL use
    }
}
```

**Action**: Investigate if any WebGL-specific GL context management exists.

#### 4. Window/WebView Management

**Status**: ✅ No WebGL-specific code

WebView and Window structs handle all canvas types generically through the compositor's image key system.

## Feature Propagation

The `webgl` feature correctly propagates to:

✅ `script/webgl` - Enables WebGL DOM APIs (HTMLCanvasElement.getContext('webgl'))
✅ `canvas/webgl` - Enables canvas WebGL backend
✅ `dep:webgpu` - Includes WebGL context implementation
✅ `dep:webgpu_traits` - Includes WebGL traits

## Testing Strategy

### Build Tests

```bash
# Test without webgl (default)
cargo build
cargo test

# Test with webgl
cargo build --features webgl
cargo test --features webgl
```

### Expected Behavior

**Without `webgl` feature**:
- `canvas.getContext('webgl')` returns `null`
- WebGL-related types/traits not compiled
- Smaller binary size

**With `webgl` feature**:
- `canvas.getContext('webgl')` creates WebGL context
- Full WebGL 1.0 API available
- WebGL contexts composited through WebRender

## Current Status

| Task | Status | Notes |
|------|--------|-------|
| A1.1: Add feature flag | ✅ Complete | Merged in commit 4e6cadf |
| A1.2: Feature guards | 🚧 In Progress | Most code already in Servo upstream |
| A1.3: Context creation | ⏳ Pending | Depends on A1.2 |
| A1.4: Testing | ⏳ Pending | Depends on A1.3 |

## Notes

### Why No Commented Code?

Unlike the original assumption, verso-green doesn't have commented-out WebGL code. Instead:

1. **Servo Dependency**: WebGL implementation lives in Servo's crates, not verso-green
2. **Conditional Compilation**: Servo already uses feature flags for WebGL
3. **Generic Compositor**: The compositor treats WebGL canvases as generic external images

### Implementation Approach

Rather than uncommenting code, A1.2 focuses on:

1. ✅ Verifying feature flag propagation
2. ✅ Ensuring optional dependencies are correctly configured
3. 🔍 Adding local feature guards only where necessary
4. ⏳ Testing that WebGL works with the feature enabled

## Next Steps

1. **A1.2 Completion**:
   - Verify no local WebGL code needs feature guards
   - Document that WebGL code is in Servo dependencies
   - Close issue #18

2. **A1.3 - Context Creation**:
   - Test that `canvas.getContext('webgl')` works
   - Verify WebGL textures composite correctly
   - Ensure resource cleanup works

3. **A1.4 - Testing**:
   - Add WebGL rendering tests
   - Test context lifecycle
   - Verify integration with WebRender

## References

- [Servo Canvas Documentation](https://book.servo.org/design-documentation/canvas.html)
- [WebGL Implementation in Servo](https://github.com/servo/servo/tree/main/components/canvas)
- [surfman - WebGL Context Manager](https://github.com/servo/surfman)
