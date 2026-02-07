# Issue #47: Update function/method signature changes across servo API

## Summary

Multiple servo API functions and methods have changed their parameter counts or types. This issue covers all signature-level changes not already tracked in other issues.

## Changed Signatures

### webrender API

#### `generate_frame` (compositor.rs:984)
- **Before**: 3 args `(id, present, reason)`
- **After**: 4 args `(id, present, tracked: bool, reasons)`

#### `hit_test` (compositor.rs:1503)
- **Before**: 4 args `(document, pipeline_id, world_point, flags)`
- **After**: 2 args `(document, world_point)`

#### `new_frame_ready` trait method (verso.rs:1116)
- **Before**: 5 params `(&self, document_id, scrolled, composite_needed, frame_publish_id)`
- **After**: 4 params `(&self, document_id, frame_publish_id, &FrameReadyParams)`

### Servo crate APIs

#### `WebViewId::new()` (window.rs:260,280; webview.rs:539; context_menu.rs:86; history_menu.rs:31; prompt.rs:99)
- **Before**: 0 args
- **After**: 1 arg `(PainterId)`

#### `SystemFontService::spawn` (verso.rs:281)
- **Before**: 1 arg `(cross_process_paint_api)`
- **After**: 2 args `(cross_process_paint_api, mem_profiler_chan)`

#### `CanvasPaintThread::start` (verso.rs:286)
- **Before**: 3 args `(cross_process_paint_api, system_font_service, public_resource_threads)`
- **After**: 1 arg `(cross_process_paint_api)`

#### `resource_thread::new_resource_threads` (verso.rs:267-277)
- **Before**: returns `(public, private)` — 2-tuple
- **After**: returns `(public, private, async_runtime)` — 3-tuple
- Also requires `GenericEmbedderProxy<NetToEmbedderMsg>` instead of `GenericEmbedderProxy<EmbedderMsg>` for the embedder_proxy arg

#### `Constellation::start` (verso.rs:319-327)
- **Before**: 7 args including `canvas_create_sender` and `canvas_ipc_sender`
- **After**: 6 args, needs `Receiver<EmbedderToConstellationMessage>` as first arg, removes canvas args

#### `webdriver_server::start_server` (verso.rs:331)
- **Before**: 2 args `(port, constellation_sender)`
- **After**: 4 args `(port, sender, waker, preferences)`

#### `BluetoothThreadFactory::new` (verso.rs:264)
- **Before**: implemented for `IpcSender<BluetoothRequest>`
- **After**: implemented for `GenericSender<BluetoothRequest>`

### Constructor changes

#### `CrossProcessPaintApi` (verso.rs:1185)
- Constructor is now private. Use `CrossProcessPaintApi::new()` or `CrossProcessPaintApi::dummy()` instead.

## Affected Files

- `src/compositor.rs`
- `src/verso.rs`
- `src/window.rs`
- `src/webview/webview.rs`
- `src/webview/context_menu.rs`
- `src/webview/history_menu.rs`
- `src/webview/prompt.rs`

## Proposed Fix

Update each call site to match the new API signatures. Reference the servo source at rev `e8aa7d51` for the correct signatures.

## Error Count

~25 errors

## Labels

`bug`, `api-change`
