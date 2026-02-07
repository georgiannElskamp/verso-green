# Issue #44: Update PaintMessage enum variant handling in compositor.rs

## Summary

The `PaintMessage` enum in the `paint_api` crate has undergone significant changes. Many variants have been removed, renamed, or had their field signatures changed. This causes ~25 errors in `src/compositor.rs`.

## Removed Variants (no longer exist)

| Variant | Line | Status |
|---------|------|--------|
| `PaintMessage::CreateOrUpdateWebView` | 583 | Removed |
| `PaintMessage::RemoveWebView` | 588 | Removed |
| `PaintMessage::TouchEventProcessed` | 592 | Removed |
| `PaintMessage::CreatePng` | 596 | Removed |
| `PaintMessage::IsReadyToSaveImageReply` | 603 | Removed |
| `PaintMessage::LoadComplete` | 641 | Removed |
| `PaintMessage::WebDriverMouseButtonEvent` | 648 | Removed |
| `PaintMessage::WebDriverMouseMoveEvent` | 661 | Removed |
| `PaintMessage::SendScrollNode` | 678 | Removed |
| `PaintMessage::HitTest` | 793 | Removed |
| `PaintMessage::AddImage` | 871 | Removed |
| `PaintMessage::GetClientWindowRect` | 891, 955 | Removed |
| `PaintMessage::GetScreenSize` | 900, 960 | Removed |
| `PaintMessage::GetAvailableScreenSize` | 909, 965 | Removed |

## Changed Variants

### `SendInitialTransaction`
- **Before**: 1 field `(pipeline)`
- **After**: 2 fields `(WebViewId, WebRenderPipelineId)`
- Lines: 670

### `GenerateImageKey`
- **Before**: 1 field `(sender)`
- **After**: 2 fields `(WebViewId, GenericSender<ImageKey>)`
- Lines: 810, 938

### `SendDisplayList`
- **Before**: had `display_list_receiver` field
- **After**: has `display_list_info_receiver` and `display_list_data_receiver` instead
- Lines: 710-714

### `NewWebRenderFrameReady`
- **Before**: 2 fields `(DocumentId, composite_needed)`
- **After**: 3 fields `(PainterId, DocumentId, bool)`
- Lines: 1123 (in verso.rs)

## Related Changes

- `PipelineExitSource::send()` method no longer exists (lines 624, 936)
- `ScrollTreeNode::set_offset()` removed, replaced by `offset()` with different args (lines 402, 1282)
- `set_scroll_offsets_for_node_with_external_scroll_id` renamed to `set_scroll_offset_for_node_with_external_scroll_id` (singular, different args) (line 688)

## Affected File

- `src/compositor.rs` — primary file affected

## Proposed Fix

Audit the current `PaintMessage` enum definition at the pinned servo rev (`e8aa7d51`) and update all match arms in `compositor.rs` to match the new API. Removed variants likely have replacement patterns in the new servo compositor code.

## Error Count

~25 errors

## Labels

`bug`, `api-change`, `compositor`
