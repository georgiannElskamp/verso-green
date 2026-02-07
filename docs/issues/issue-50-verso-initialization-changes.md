# Issue #50: Update Verso initialization and configuration code

## Summary

The `Verso::new()` initialization in `src/verso.rs` and configuration code in `src/config.rs` have multiple errors due to changed servo APIs for browser setup, resource thread creation, constellation initialization, and webdriver configuration.

## Configuration Issues (config.rs)

### `set_options(opts)` removed (line 409)
The `servo_config::opts::set_options()` function no longer exists. Need to find the new way to initialize servo options.

### `Preferences` struct changes (line 420)
`dom_svg_enabled` field doesn't exist. Compiler suggests `dom_webgpu_enabled`.

### `Resource::RippyPNG` removed (line 458)
Resource variant no longer exists.

### `Response` API changes (lines 521, 525)
- `response.body.lock().unwrap()` — parking_lot mutex doesn't use `unwrap()`, use `response.body.lock()` directly
- `Response::network_internal_error(msg)` — use `Response::network_error(msg)` instead

## Initialization Issues (verso.rs)

### Removed/renamed imports (lines 13-14, 22, 38)
- `WebrenderExternalImageHandlers` → `WebRenderExternalImageHandlers`
- `WebrenderImageHandlerType` → `WebRenderImageHandlerType`
- `user_contents::UserContentManager` → doesn't exist (only `UserContentManagerId`)
- `use webgpu;` — not available as standalone crate import

### `DiagnosticsLogging` field changes (lines 150-190)
- `disable_share_style_cache` → removed
- `dump_style_statistics` → `style_statistics`
- `webrender_stats` → removed
- `convert_mouse_to_touch` → removed

### `Opts` field changes (lines 203, 330, 352)
- `shaders_dir` → removed
- `webdriver_port` → removed
- `wait_for_stable_image` → removed

### External image handlers type inference (line 232)
Need explicit type annotation for `Box<_>` around `external_image_handlers`.

### `BluetoothThreadFactory` impl change (line 264)
Now implemented for `GenericSender<BluetoothRequest>`, not `IpcSender<BluetoothRequest>`.

### `resource_thread::new_resource_threads` changes (lines 267-277)
- Returns 3-tuple `(public, private, async_runtime)` instead of 2-tuple
- Requires `GenericEmbedderProxy<NetToEmbedderMsg>` instead of `GenericEmbedderProxy<EmbedderMsg>`

### `SystemFontService::spawn` (line 281)
Now takes 2 args (added `ProfilerChan`).

### `CanvasPaintThread::start` (line 286)
Now takes 1 arg instead of 3 (removed `system_font_service` and `public_resource_threads`).

### `InitialConstellationState` fields (lines 300-314)
Removed: `compositor_proxy`, `webrender_document`, `webrender_api_sender`, `webrender_external_images`, `user_contents`
Available: `paint_proxy`, `public_storage_threads`, `private_storage_threads`, `webrender_external_image_id_manager`, `privileged_urls`, `async_runtime`

### `Constellation::start` (lines 319-327)
New arg list: needs `Receiver<EmbedderToConstellationMessage>` first, removed canvas args.

### `webdriver_server::start_server` (line 331)
Now takes 4 args: `(port, sender, waker, preferences)`.

### Constellation sender type (lines 343, 361, 363, 388)
`constellation_sender` is `()` instead of `Sender<EmbedderToConstellationMessage>`. The constellation start function likely now returns the sender differently.

### `CrossProcessPaintApi` constructor (line 1185)
Constructor is private. Use `CrossProcessPaintApi::new()` or `CrossProcessPaintApi::dummy()`.

### Paint message receiver type (line 1200)
Expected `Receiver<PaintMessage>`, got `Receiver<Result<PaintMessage, Box<ErrorKind>>>`.

## Affected Files

- `src/config.rs`
- `src/verso.rs`

## Proposed Fix

This requires a comprehensive rewrite of the `Verso::new()` function and `Config` initialization to match the new servo initialization API. Key references:
- Check servo's own embedder code at rev `e8aa7d51` for initialization patterns
- Review `InitialConstellationState` available fields
- Review `Constellation::start` new signature

## Error Count

~40 errors

## Priority

**Critical** — the application cannot start without these fixes.

## Labels

`bug`, `api-change`, `initialization`
