# Build Error Tracking: PR Fast Check Job

This document tracks all 234 build errors from the PR Fast Check CI job,
categorized into 7 distinct issues with corresponding fix branches.

## Issue Summary

| # | Branch | Description | ~Errors | Priority |
|---|--------|-------------|---------|----------|
| 1 | `claude/issue-1-syntax-and-module-I1hTQ` | Fix trailing backslash + register scroll_coalescing | 2 | P0 (blocking) |
| 2 | `claude/issue-2-dependency-versions-I1hTQ` | Resolve dependency version conflicts | ~60 | P0 (root cause) |
| 3 | `claude/issue-3-compositor-api-I1hTQ` | Update compositor.rs for servo API changes | ~70 | P1 |
| 4 | `claude/issue-4-verso-init-api-I1hTQ` | Update verso.rs initialization + EmbedderMsg | ~30 | P1 |
| 5 | `claude/issue-5-window-input-api-I1hTQ` | Update window.rs input event API | ~25 | P1 |
| 6 | `claude/issue-6-webview-module-api-I1hTQ` | Update webview module dialogs/menus | ~40 | P1 |
| 7 | `claude/issue-7-config-touch-api-I1hTQ` | Update config.rs + touch.rs API | ~7 | P2 |

**Dependency order**: Issue 1 -> Issue 2 -> Issues 3-7 (can be parallel after 2)

---

## Issue 1: Fix trailing backslash and register scroll_coalescing module

**Branch**: `claude/issue-1-syntax-and-module-I1hTQ`
**Files**: `src/compositor.rs`, `src/lib.rs`
**Errors**: 2

### Errors

1. `src/compositor.rs:50` — Trailing `\` character at end of import line:
   ```
   error: unknown start of token: \
   ```
2. `src/compositor.rs:52` — `scroll_coalescing` module not declared in `src/lib.rs`:
   ```
   error[E0432]: unresolved import `crate::scroll_coalescing`
   ```

### Fix
- Remove `\` at end of line 50 in `src/compositor.rs`
- Add `pub mod scroll_coalescing;` to `src/lib.rs`

---

## Issue 2: Resolve dependency version conflicts in Cargo.toml

**Branch**: `claude/issue-2-dependency-versions-I1hTQ`
**Files**: `Cargo.toml`, `Cargo.lock`
**Errors**: ~60 (type mismatches from dual crate versions)

### Root Cause

Three dependencies have version conflicts between direct deps and transitive deps from servo:

| Crate | Direct Version | Servo Transitive Version | Conflict |
|-------|---------------|-------------------------|----------|
| `webrender_api` | git branch `0.66` | `0.68.0` (crates.io) | Types: `ExternalScrollId`, `PipelineId`, `Epoch`, `DevicePixel`, `LayoutPixel`, `FontKey`, `FontInstanceKey`, `ImageKey`, `NativeFontHandle`, `FontInstanceFlags`, `BuiltDisplayListDescriptor` |
| `ipc-channel` | `0.19` | `0.20.2` | Types: `IpcSender`, `IpcSharedMemory` |
| `keyboard-types` | `0.7` | `0.8.3` | Types: `CompositionEvent`, `KeyboardEvent` |

### Errors (representative)
- `two different versions of crate webrender_api are being used` (~40 errors)
- `two different versions of crate ipc_channel are being used` (~10 errors)
- `expected CompositionEvent, found a different CompositionEvent` (~5 errors)
- `expected KeyboardEvent, found a different KeyboardEvent` (~5 errors)

### Fix
- Update `ipc-channel` from `0.19` to match servo's transitive dep version
- Update `keyboard-types` from `0.7` to match servo's transitive dep version
- Align `webrender`/`webrender_api` git source to match what servo rev `e8aa7d5` expects

---

## Issue 3: Update compositor.rs for upstream servo API changes

**Branch**: `claude/issue-3-compositor-api-I1hTQ`
**Files**: `src/compositor.rs`
**Errors**: ~70

### Categories

#### 3a. Removed PaintMessage variants
These PaintMessage variants no longer exist:
- `CreateOrUpdateWebView`, `RemoveWebView`, `TouchEventProcessed`
- `CreatePng`, `IsReadyToSaveImageReply`, `LoadComplete`
- `WebDriverMouseButtonEvent`, `WebDriverMouseMoveEvent`
- `SendScrollNode`, `HitTest`
- `AddImage`, `GetClientWindowRect`, `GetScreenSize`, `GetAvailableScreenSize`

#### 3b. Changed PaintMessage variants
- `SendInitialTransaction` — now takes 2 fields (added `WebViewId`)
- `GenerateImageKey` — now takes 2 fields (added `WebViewId`)
- `SendDisplayList` — field `display_list_receiver` replaced with `display_list_info_receiver` + `display_list_data_receiver`
- `ImageUpdate::AddImage` / `UpdateImage` — now take 4 fields (added `bool`/`Option<Epoch>`)
- `NewWebRenderFrameReady` — now takes 3 fields (added `PainterId`)

#### 3c. Removed/changed types
- `HitTestInfo` — no longer exists
- `ScrollState` — no longer exists
- `AnimationTickType` — no longer exists
- `ProfilerCategory::Compositing` — removed

#### 3d. Changed struct fields
- `PaintHitTestResult` — fields `cursor`, `point_relative_to_item`, `node`, `scroll_tree_node` removed
- `PipelineDetails.hit_test_items` — field removed
- `PaintDisplayListInfo.hit_test_info` — field removed

#### 3e. Changed methods
- `ScrollTreeNode::set_offset` — renamed/changed signature
- `PipelineExitSource::send` — removed
- `ScrollTree::set_scroll_offsets_for_node_with_external_scroll_id` → `set_scroll_offset_for_node_with_external_scroll_id`
- `Epoch::as_u16()` — removed

#### 3f. Input event changes affecting compositor
- `MouseMoveEvent` — now requires `is_compatibility_event_for_touch` field
- `MouseButtonAction::Click` — removed
- `WebViewPoint` — many places pass `DevicePoint` where `WebViewPoint` enum is now expected
- `ForwardInputEvent` — now takes `InputEventAndId` instead of `InputEvent`
- `EmbedderToConstellationMessage::SetCursor` — removed
- `EmbedderToConstellationMessage::TickAnimation` — changed signature
- `EmbedderToConstellationMessage::IsReadyToSaveImage` — removed

#### 3g. Epoch/PipelineId conversion issues
- `base::Epoch` vs `webrender_api::Epoch` mismatches
- `PipelineId` / `NamespaceIndex<PipelineIndex>` conversion issues

---

## Issue 4: Update verso.rs for initialization and API changes

**Branch**: `claude/issue-4-verso-init-api-I1hTQ`
**Files**: `src/verso.rs`
**Errors**: ~30

### Categories

#### 4a. Import fixes
- `WebrenderExternalImageHandlers` → `WebRenderExternalImageHandlers`
- `WebrenderImageHandlerType` → `WebRenderImageHandlerType`
- `UserContentManager` → `UserContentManagerId`
- `use webgpu;` — needs to be feature-gated with `#[cfg(feature = "webgl")]`
- `use layout::LayoutFactoryImpl` — unused import

#### 4b. Removed/changed configuration
- `set_options(opts)` — function removed
- `Preferences::dom_svg_enabled` — field removed
- `Opts.shaders_dir` — field removed
- `Opts.webdriver_port` — field removed
- `Opts.wait_for_stable_image` — field removed
- `DiagnosticsLogging.disable_share_style_cache` — field removed
- `DiagnosticsLogging.dump_style_statistics` → `style_statistics`
- `DiagnosticsLogging.webrender_stats` — field removed
- `DiagnosticsLogging.convert_mouse_to_touch` — field removed

#### 4c. Changed initialization APIs
- `CrossProcessPaintApi` — constructor now private, use `::new()` or `::dummy()`
- `InitialConstellationState` — removed fields: `compositor_proxy`, `webrender_document`, `webrender_api_sender`, `webrender_external_images`, `user_contents`
- `Constellation::start` — now takes 6 args (changed signature, removed canvas args)
- `SystemFontService::spawn` — now takes 2 args (added `ProfilerChan`)
- `CanvasPaintThread::start` — now takes 1 arg (removed font service and resource threads)
- `resource_thread::new_resource_threads` — returns 3-tuple now (added `Box<dyn AsyncRuntime>`)
- `BluetoothThreadFactory::new` — trait implementation changed
- `webdriver_server::start_server` — now takes 4 args

#### 4d. Changed EmbedderMsg handling
- `EmbedderMsg::RequestAuthentication` — removed
- `EmbedderMsg::ShowContextMenu` — removed
- `EmbedderMsg::Keyboard` — removed
- `EmbedderMsg::WebResourceRequested` — removed
- `EmbedderMsg::SelectFiles` — removed
- `EmbedderMsg::ShowIME` — removed
- `EmbedderMsg::HideIME` — removed
- `EmbedderMsg::PlayGamepadHapticEffect` — removed
- `EmbedderMsg::StopGamepadHapticEffect` — removed
- `EmbedderMsg::ShowSelectElementMenu` — removed
- `EmbedderMsg::WebViewFocused` — now has 2 fields (added `bool`)

#### 4e. Other changes
- `constellation_sender` type errors (returns `()` instead of `Sender`)
- Type annotation needed for `external_image_handlers`

---

## Issue 5: Update window.rs for input event API changes

**Branch**: `claude/issue-5-window-input-api-I1hTQ`
**Files**: `src/window.rs`
**Errors**: ~25

### Categories

#### 5a. Input event type changes
- `MouseMoveEvent` — needs `is_compatibility_event_for_touch` field
- `MouseButtonAction::Click` — removed
- Multiple places need `WebViewPoint` wrapping instead of raw `Point2D<f32, DevicePixel>`
- `ForwardInputEvent` — takes `InputEventAndId` not `InputEvent`

#### 5b. Keyboard/composition type mismatches
- `CompositionEvent` — keyboard-types 0.7 vs 0.8 version conflict
- `KeyboardEvent` — keyboard-types 0.7 vs 0.8 version conflict

#### 5c. Window API changes
- `WebViewId::new()` — now takes `PainterId`
- `NewWebView` — now takes `NewWebViewDetails` instead of separate args
- `AlertResponse::default()`, `ConfirmResponse::default()`, `PromptResponse::default()` — removed
- `SharedRasterImage` — `width`/`height` moved to `metadata`, `bytes()` is now a field

---

## Issue 6: Update webview module for EmbedderMsg and dialog API changes

**Branch**: `claude/issue-6-webview-module-api-I1hTQ`
**Files**: `src/webview/webview.rs`, `src/webview/context_menu.rs`, `src/webview/history_menu.rs`, `src/webview/prompt.rs`, `src/webview/webview_menu.rs`
**Errors**: ~40

### Categories

#### 6a. EmbedderMsg variant changes (affects webview.rs)
- `WebResourceRequested` — removed
- `ShowContextMenu` — removed
- `RequestAuthentication` — removed
- `SelectFiles` — removed
- `ShowIME` — removed
- `HideIME` — removed
- `WebViewFocused` — now has 2 fields

#### 6b. Dialog/prompt type changes
- `SimpleDialog` — type no longer exists or changed
- `ContextMenuResult` — type removed
- `ContextMenuRequest::Dismissed` — removed
- `AlertResponse::default()` — removed
- `ConfirmResponse::default()` — removed
- `PromptResponse::default()` — removed

#### 6c. Navigation API changes
- `TraverseHistory` — now takes 3 args (added `TraversalId`)
- `NewWebView` — now takes `NewWebViewDetails`
- `WebViewId::new()` — now takes `PainterId`

#### 6d. Other changes
- `WebDriverScriptCommand::ExecuteScript` — removed
- `WebViewId.0` — field is now private
- `PromptSender::AllowDenySender` — type mismatch (`IpcSender` vs `GenericSender`)
- `DevicePixel` version conflicts in ViewportDetails

---

## Issue 7: Update config.rs and touch.rs for API changes

**Branch**: `claude/issue-7-config-touch-api-I1hTQ`
**Files**: `src/config.rs`, `src/touch.rs`
**Errors**: ~7

### config.rs errors
- `set_options(opts)` — function removed
- `Preferences::dom_svg_enabled` — field removed
- `Resource::RippyPNG` — variant removed
- `response.body.lock().unwrap()` — `MutexGuard` no longer has `unwrap()`
- `Response::network_internal_error` → `Response::network_error`

### touch.rs errors
- `InputEventResult::DefaultPrevented(_, _)` — no longer a tuple variant (now a constant)
- `InputEventResult::DefaultAllowed` — removed
