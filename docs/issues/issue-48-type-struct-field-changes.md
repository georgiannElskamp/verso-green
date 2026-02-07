# Issue #48: Fix struct/type field changes and type mismatches

## Summary

Several structs and types in the servo ecosystem have had fields added, removed, or renamed. Additionally, some types have been replaced with different types entirely. This issue tracks all field-level and type-level changes.

## Struct Field Changes

### `PaintHitTestResult` (compositor.rs)
Fields removed:
- `cursor` (line 472) — no longer available
- `node` (line 1528) — removed
- `scroll_tree_node` (line 1530, 1797) — removed
- `point_relative_to_item` (line 1527) — removed
Remaining fields: `pipeline_id`, `point_in_viewport`, `external_scroll_id`

### `MouseMoveEvent` (compositor.rs:666, window.rs:452, compositor.rs:1622)
- Missing field: `is_compatibility_event_for_touch` — must be provided in all `MouseMoveEvent` initializers

### `MouseButtonAction` enum (compositor.rs:1436,1453,1643; window.rs:538)
- `Click` variant removed — need to find replacement pattern

### `Preferences` struct (config.rs:420)
- `dom_svg_enabled` field removed — use `dom_webgpu_enabled` or remove

### `Opts` struct (verso.rs)
- `shaders_dir` field removed (line 203)
- `webdriver_port` field removed (line 330)
- `wait_for_stable_image` field removed (line 352)

### `DiagnosticsLogging` struct (verso.rs)
- `disable_share_style_cache` field removed (line 150)
- `dump_style_statistics` field removed (line 152) — use `style_statistics`
- `webrender_stats` field removed (line 190)
- `convert_mouse_to_touch` field removed (line 353)

### `InitialConstellationState` struct (verso.rs)
Fields removed:
- `compositor_proxy` (line 300) — use `paint_proxy`
- `webrender_document` (line 309)
- `webrender_api_sender` (line 310)
- `webrender_external_images` (line 313) — use `webrender_external_image_id_manager`
- `user_contents` (line 314)

### `SharedRasterImage` / favicon (window.rs:1010)
- No direct `width`/`height` fields — use `metadata.width`/`metadata.height`
- No `bytes()` method — `bytes` is a field, not a method

### `ImageUpdate::AddImage` / `UpdateImage` (compositor.rs:818,823)
- **Before**: 3 fields `(key, desc, data)`
- **After**: 4 fields `(key, desc, data, is_animated_image)`

## Type Replacement Changes

### `WebViewPoint` vs `DevicePoint` (compositor.rs, window.rs)
Input events now use `WebViewPoint` enum instead of `Point2D<f32, DevicePixel>`:
- compositor.rs: lines 654, 1416, 1420, 1537, 1569, 1574, 1577, 1608, 1609, 1615, 1622, 1628, 1636, 1644
- window.rs: lines 508, 515, 537, 452

### `InputEvent` → `InputEventAndId` (compositor.rs:1426,1546; window.rs:1200)
`ForwardInputEvent` now expects `InputEventAndId` instead of `InputEvent`. Use `.into()` to convert.

### `ScrollEvent` type mismatch (compositor.rs:1587,1678,1681,1689)
`compositor::ScrollEvent` and `scroll_coalescing::ScrollEvent` are now distinct types.

### `PipelineId` ↔ `NamespaceIndex<PipelineIndex>` (compositor.rs)
Conversion issues between these types at lines 1115, 1509, 1808, 1859, 1980, 2251.

### `IpcSender` → `GenericSender` (webview.rs:370)
`PromptSender::AllowDenySender` expects `IpcSender<AllowOrDeny>` but receives `GenericSender<AllowOrDeny>`.

### `keyboard_types::KeyboardEvent` → `embedder_traits::KeyboardEvent` (window.rs:682)
Servo now uses its own `KeyboardEvent` type.

## Response Type Changes

### `AlertResponse::default()` (window.rs:1046; webview.rs:860,870)
No `default()` method. Use `default_color()` or the appropriate constructor.

### `ConfirmResponse::default()` (window.rs:1049; webview.rs:878)
No `default()` method.

### `PromptResponse::default()` (window.rs:1052; webview.rs:521,530,577,625,630,802,896,901)
No `default()` method.

## Affected Files

- `src/compositor.rs`
- `src/verso.rs`
- `src/config.rs`
- `src/window.rs`
- `src/webview/webview.rs`
- `src/webview/prompt.rs`
- `src/webview/webview_menu.rs`

## Error Count

~60 errors

## Labels

`bug`, `api-change`
