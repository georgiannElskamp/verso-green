# Issue #45: Update EmbedderMsg and EmbedderToConstellationMessage API usage

## Summary

The `EmbedderMsg` enum and `EmbedderToConstellationMessage` enum in servo's `embedder_traits` and `constellation_traits` crates have undergone significant changes. Multiple variants have been removed, renamed, or had their signatures changed. This affects `src/verso.rs`, `src/webview/webview.rs`, `src/window.rs`, and related files.

## Removed EmbedderMsg Variants

| Variant | File:Line | Status |
|---------|-----------|--------|
| `EmbedderMsg::RequestAuthentication` | verso.rs:690, webview.rs:377 | Removed |
| `EmbedderMsg::ShowContextMenu` | verso.rs:691, webview.rs:278,755,823,941 | Removed |
| `EmbedderMsg::Keyboard` | verso.rs:698 | Removed |
| `EmbedderMsg::WebResourceRequested` | verso.rs:707, webview.rs:189 | Removed |
| `EmbedderMsg::SelectFiles` | verso.rs:710, webview.rs:392 | Removed |
| `EmbedderMsg::ShowIME` | verso.rs:712, webview.rs:428 | Removed |
| `EmbedderMsg::HideIME` | verso.rs:713, webview.rs:437 | Removed |
| `EmbedderMsg::PlayGamepadHapticEffect` | verso.rs:718 | Removed |
| `EmbedderMsg::StopGamepadHapticEffect` | verso.rs:719 | Removed |
| `EmbedderMsg::ShowSelectElementMenu` | verso.rs:721 | Removed |

## Changed EmbedderMsg Variants

### `WebViewFocused`
- **Before**: 1 field `(WebViewId)`
- **After**: 2 fields `(WebViewId, bool)`
- Lines: verso.rs:695, webview.rs:100,468,793,852

## Removed/Changed EmbedderToConstellationMessage Variants

| Variant | File:Line | Status |
|---------|-----------|--------|
| `SetCursor` | compositor.rs:490 | Removed |
| `IsReadyToSaveImage` | compositor.rs:1989 | Removed |

### `NewWebView`
- **Before**: 3 args `(ServoUrl, WebViewId, ViewportDetails)`
- **After**: 2 args `(ServoUrl, NewWebViewDetails)`
- Lines: webview.rs:558, prompt.rs:272, webview_menu.rs:39, window.rs:270,311

### `TraverseHistory`
- **Before**: 2 args `(WebViewId, TraversalDirection)`
- **After**: 3 args `(WebViewId, TraversalDirection, TraversalId)`
- Lines: webview.rs:702,712, context_menu.rs:345,354, history_menu.rs:202,211

### `ForwardInputEvent`
- **Before**: accepts `InputEvent`
- **After**: accepts `InputEventAndId`
- Lines: compositor.rs:1426, window.rs:1200

### `TickAnimation`
- **Before**: `(pipeline_id, tick_type)`
- **After**: `(Vec<WebViewId>)` — single arg
- Lines: compositor.rs:1859

## Removed Types

| Type | File:Line |
|------|-----------|
| `ContextMenuResult` | webview.rs:284,761,828,942 |
| `SimpleDialog` | webview.rs:300,312,324,502,797,856 |
| `UserContentManager` | verso.rs:22 |
| `WebDriverJSValue` (use `WebDriverJSResult`) | window.rs:9 |

## Affected Files

- `src/verso.rs`
- `src/webview/webview.rs`
- `src/webview/context_menu.rs`
- `src/webview/history_menu.rs`
- `src/webview/prompt.rs`
- `src/webview/webview_menu.rs`
- `src/window.rs`
- `src/compositor.rs`

## Proposed Fix

1. Audit the current servo `embedder_traits` and `constellation_traits` at rev `e8aa7d51` to find the new API surface
2. Remove handling for removed variants — these events may no longer be sent
3. Update `NewWebView` calls to use `NewWebViewDetails` struct
4. Update `TraverseHistory` calls to include `TraversalId`
5. Wrap `InputEvent` in `InputEventAndId` where needed (use `.into()`)
6. Replace `SimpleDialog` with the new dialog API
7. Replace `ContextMenuResult` with the new context menu API

## Error Count

~35 errors

## Labels

`bug`, `api-change`, `embedder`
