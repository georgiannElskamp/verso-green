# Issue #46: Fix keyboard Key enum — named variants removed from keyboard-types

## Summary

The `keyboard_types::Key` enum has been restructured. All named key variants (like `Key::Escape`, `Key::F1`, `Key::Enter`, etc.) have been removed. The `get_servo_key_from_winit_key` function in `src/keyboard.rs` uses a macro that maps `NamedKey` variants to `Key` variants, but the `Key` enum no longer has these named variants.

## Affected Variants (all removed from `Key` enum)

**Function keys**: `Escape`, `F1`-`F12`

**Navigation**: `PrintScreen`, `Pause`, `Insert`, `Home`, `Delete`, `End`, `PageDown`, `PageUp`, `ArrowLeft`, `ArrowUp`, `ArrowRight`, `ArrowDown`

**Editing**: `Backspace`, `Enter`, `Tab`, `Copy`, `Paste`, `Cut`

**Modifiers**: `Alt`, `Control`, `Shift`, `Meta`, `CapsLock`, `NumLock`

**Composition**: `Compose`, `Convert`, `NonConvert`, `KanaMode`, `KanjiMode`

**Media**: `MediaStop`, `MediaPlayPause`, `MediaTrackNext`, `MediaTrackPrevious`, `AudioVolumeMute`, `AudioVolumeDown`, `AudioVolumeUp`

**Browser**: `BrowserBack`, `BrowserFavorites`, `BrowserForward`, `BrowserHome`, `BrowserRefresh`, `BrowserSearch`, `BrowserStop`

**System**: `Power`, `Standby`, `WakeUp`, `LaunchApplication1`, `LaunchApplication2`, `LaunchMail`

**Special**: `Unidentified`

## Error Location

`src/keyboard.rs` lines 48-108, specifically the `logical_to_winit_key!` macro invocation in `get_servo_key_from_winit_key()`.

Total: ~50 individual errors from this single macro expansion.

## Proposed Fix

The `keyboard_types` crate (v0.8.3) likely restructured the `Key` enum. The fix should either:

1. **Update the mapping** to use the new `Key` enum structure — possibly `Key::Named(NamedKey::...)` or a similar wrapper pattern
2. **Use `embedder_traits::KeyboardEvent`** instead of `keyboard_types::KeyboardEvent` — the error at `window.rs:682` shows these are now different types. The servo `embedder_traits` defines its own `KeyboardEvent` that wraps the key info differently
3. **Convert from winit keys directly to embedder_traits types** bypassing `keyboard_types` if it's no longer the intermediary

## Related Error

```
error[E0308]: mismatched types (window.rs:682)
  expected `embedder_traits::KeyboardEvent`, found `keyboard_types::KeyboardEvent`
```

This suggests servo now uses its own `KeyboardEvent` type. The keyboard mapping may need to target `embedder_traits::KeyboardEvent` instead.

## Error Count

~50 errors

## Priority

High — keyboard input is entirely broken without this fix.

## Labels

`bug`, `api-change`, `input`
