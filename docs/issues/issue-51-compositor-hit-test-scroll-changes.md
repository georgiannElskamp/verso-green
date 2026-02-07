# Issue #51: Update compositor hit testing, scrolling, and input dispatch

## Summary

The compositor's hit testing, scroll handling, and input event dispatching code needs significant updates to match the new servo paint/compositing API. This covers the interconnected changes in `src/compositor.rs` that affect how user input is processed, how hit tests are performed, and how scroll events are handled.

## Hit Testing Changes

### `hit_test` method (line 1503)
- **Before**: `hit_test(document, pipeline_id, world_point, flags)` — 4 args
- **After**: `hit_test(document, world_point)` — 2 args (no pipeline filter, no flags)

### `PaintHitTestResult` struct (lines 1526-1530)
Lost most fields, only retains `pipeline_id`, `point_in_viewport`, `external_scroll_id`:
- `point_in_viewport` expects `Point2D<f32, CSSPixel>` not `Point2D<f32, UnknownUnit>`
- Removed: `point_relative_to_item`, `node`, `cursor`, `scroll_tree_node`

### `HitTestResultItem` field changes (line 1527)
- `point_relative_to_item` field no longer exists on `HitTestResultItem`

### `HitTestFlags` removed (line 41)
Import and all usage must be removed.

### `HitTestInfo` removed (line 358)
Used in `PipelineDetails.hit_test_items` — this field/type needs replacement.

### Epoch conversion (line 1519)
`Epoch::as_u16()` method doesn't exist. Need alternative for comparing epochs.

### Cursor handling (lines 472-490)
- `PaintHitTestResult` no longer has a `cursor` field
- `EmbedderToConstellationMessage::SetCursor` doesn't exist
- Cursor updates need a different mechanism

## Scroll Handling Changes

### `ScrollState` removed (line 1929)
The `ScrollState` struct no longer exists. Scroll state tracking needs to be reimplemented.

### `ScrollTreeNode::set_offset` removed (lines 402, 1282)
Method replaced with `offset()` (getter only, different signature).

### `set_scroll_offsets_for_node_with_external_scroll_id` → singular (line 688)
Renamed to `set_scroll_offset_for_node_with_external_scroll_id` with different args (needs `ScrollType` parameter).

### `scroll_node_or_ancestor` (line 1806)
- **Before**: 2 args `(scroll_tree_node, scroll_location)`
- **After**: 3 args `(scroll_tree_node, scroll_location, scroll_type)`
- Also `ScrollLocation` type comes from wrong webrender_api version

### `ScrollEvent` type conflict (lines 1587, 1678, 1681, 1689)
`compositor::ScrollEvent` (private) vs `scroll_coalescing::ScrollEvent` (public) are now distinct types with incompatible fields.

## Input Dispatch Changes

### `WebViewPoint` vs `DevicePoint` (multiple lines)
All input event points now use `WebViewPoint` enum instead of raw `Point2D<f32, DevicePixel>`. Affected methods:
- `hit_test_at_point` — expects `DevicePoint` but receives `WebViewPoint`
- `update_cursor` — expects `DevicePoint` but receives `WebViewPoint`
- `on_touch_down/move/up/cancel` — expect `Point2D<f32, DevicePixel>` but receive `WebViewPoint`
- `simulate_mouse_click` — expects `DevicePoint` but receives `WebViewPoint`
- Mouse button/move events create `WebViewPoint` where `DevicePoint` expected (and vice versa)

### `MouseButtonAction::Click` removed (lines 1436, 1453, 1643)
No `Click` variant exists. Need to find the new way to represent click actions.

### `MouseMoveEvent` requires `is_compatibility_event_for_touch` (lines 666, 1622)
New required field in `MouseMoveEvent` initializer.

## Affected File

`src/compositor.rs` — all changes are in this file.

## Proposed Fix

1. Update `hit_test_at_point` to use the simplified 2-arg hit_test API
2. Reconstruct `PaintHitTestResult` from the reduced available fields
3. Implement `WebViewPoint` → `DevicePoint` conversion where needed
4. Unify the `ScrollEvent` types or add conversion
5. Update scroll methods to use new signatures
6. Handle cursor updates through the new mechanism
7. Replace `MouseButtonAction::Click` with the correct variant

## Error Count

~40 errors

## Labels

`bug`, `api-change`, `compositor`, `input`
