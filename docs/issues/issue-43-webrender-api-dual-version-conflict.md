# Issue #43: Fix webrender_api dual version conflict causing ~60 type mismatch errors

## Summary

Two different versions of `webrender_api` are being pulled into the dependency tree, causing widespread type mismatch errors across `compositor.rs` and `webview/` files. This is the **root cause** of approximately 60 of the 302 total compilation errors.

## Root Cause

- **Direct dependency**: `webrender_api` from `git = "https://github.com/servo/webrender", branch = "0.68"` (the git checkout version)
- **Transitive dependency**: `webrender_api 0.68.0` from `crates.io` pulled in via `base` → `servo_malloc_size_of` → `webrender_api`

The compiler reports:
```
note: two different versions of crate `webrender_api` are being used
  /home/runner/.cargo/registry/src/.../webrender_api-0.68.0/src/...  (crates.io version)
  /home/runner/.cargo/git/checkouts/webrender-.../webrender_api/src/...  (git version)
```

## Affected Types

All of the following types exist in both versions, causing "expected X, found a different X" errors:

- `ExternalScrollId` — scroll offset tracking in compositor
- `PipelineId` / `WebRenderPipelineId` — pipeline identification
- `FontKey` / `FontInstanceKey` / `NativeFontHandle` / `FontInstanceFlags` — font resource management
- `ImageKey` — image resource management
- `DevicePixel` / `LayoutPixel` — coordinate system units
- `Epoch` / `BuiltDisplayListDescriptor` — display list metadata
- `ScrollLocation` — scroll event types

## Affected Files

- `src/compositor.rs` — lines 396, 401, 763, 786, 822, 834, 838, 841, 848, 854, 855, 861, 864, 884, 887, 949, 952, 1170, 1254, 1509, 1808+
- `src/webview/webview.rs` — lines 51, 549, 563
- `src/webview/prompt.rs` — line 277
- `src/webview/webview_menu.rs` — line 44
- `src/window.rs` — line 291

## Proposed Fix

Add a `[patch.crates-io]` section to `Cargo.toml` to force all transitive dependencies to use the same git version of `webrender_api`:

```toml
[patch.crates-io]
webrender_api = { git = "https://github.com/servo/webrender", branch = "0.68" }
```

Alternatively, investigate whether `servo_malloc_size_of` can be configured to use the git version.

## Error Count

~60 errors directly caused by this issue.

## Priority

**Critical** — This must be fixed first as it blocks resolution of many other errors. Many errors in other categories are secondary effects of this dual-version conflict.

## Labels

`bug`, `dependencies`, `build`
