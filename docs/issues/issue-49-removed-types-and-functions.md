# Issue #49: Handle removed types, functions, and enum variants

## Summary

Several types, functions, and enum variants have been completely removed from the servo dependency crates. These need replacement implementations or removal of the code that uses them.

## Removed from `webrender_api`

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `HitTestFlags` | Type | compositor.rs:41 | Import and usage removed |
| `HitTestInfo` | Type | compositor.rs:358 | Used in `PipelineDetails` struct |
| `ScrollState` | Struct | compositor.rs:1929 | Used for scroll state tracking |

## Removed from `embedder_traits`

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `AnimationTickType` | Type | compositor.rs:1851,1853,1856 | Used for tick animation |

## Removed from `profile_traits`

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `ProfilerCategory::Compositing` | Variant | compositor.rs:2062 | Profiling category |

## Removed from `servo_config`

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `set_options` | Function | config.rs:409 | Options initialization |

## Removed from `embedder_traits::resources`

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `Resource::RippyPNG` | Variant | config.rs:458 | Resource file reference |

## Removed from `net_traits`

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `Response::network_internal_error` | Method | config.rs:525 | Use `Response::network_error` instead |

## Removed from `webrender_api` / epoch

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `Epoch::as_u16()` | Method | compositor.rs:1519 | No longer available on Epoch |

## Removed from `paint_api`

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `PaintDisplayListInfo::hit_test_info` | Field | compositor.rs:769 | Field no longer exists |

## Removed crate

| Crate | Location | Notes |
|-------|----------|-------|
| `webgpu` (as external crate) | verso.rs:38 | `use webgpu;` — not available as standalone import when feature disabled |

## Removed from `embedder_traits` / `webdriver`

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `WebDriverScriptCommand::ExecuteScript` | Variant | webview.rs:1041 | Use updated command |
| `WebViewId.0` field access | Privacy | webview.rs:1040 | Field is now private |

## Removed from `touch`

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `InputEventResult::DefaultPrevented` (tuple variant) | Pattern | touch.rs:220 | Now an associated constant, not tuple |
| `InputEventResult::DefaultAllowed` | Variant | touch.rs:221 | No longer exists |

## Removed from `lock_api` / parking_lot

| Item | Type | Location | Notes |
|------|------|----------|-------|
| `MutexGuard::unwrap()` | Method | config.rs:521 | parking_lot MutexGuard doesn't need unwrap |

## Affected Files

- `src/compositor.rs`
- `src/config.rs`
- `src/verso.rs`
- `src/window.rs`
- `src/touch.rs`
- `src/webview/webview.rs`

## Proposed Fix

1. Remove imports/usage of deleted types
2. Replace `HitTestInfo` with the new hit test result type
3. Replace `AnimationTickType` with the new animation tick API
4. Replace `set_options` with the new configuration method
5. Remove `Resource::RippyPNG` handling
6. Replace `network_internal_error` with `network_error`
7. Guard `use webgpu;` behind the `webgl` feature flag
8. Update touch event result handling to match new `InputEventResult` API
9. Remove `.unwrap()` on parking_lot `MutexGuard`

## Error Count

~20 errors

## Labels

`bug`, `api-change`, `cleanup`
