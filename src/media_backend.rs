//! Media Backend Abstraction
//!
//! This module provides media backend selection and initialization
//! with graceful fallback support.

use std::sync::Once;

/// Media backend selection
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MediaBackend {
    /// Automatically select best available backend
    #[default]
    Auto,
    /// Force GStreamer backend (fail if unavailable)
    GStreamer,
    /// Force dummy backend (no audio/video playback)
    Dummy,
}

/// Result of media backend initialization
#[derive(Debug, Clone)]
pub struct MediaInitResult {
    /// The backend that was actually initialized
    pub backend: MediaBackendType,
    /// Human-readable status message
    pub message: String,
}

/// The actual backend type that was initialized
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaBackendType {
    /// GStreamer backend (full functionality)
    GStreamer,
    /// Dummy backend (no playback)
    Dummy,
}

static INIT: Once = Once::new();
static mut INIT_RESULT: Option<MediaInitResult> = None;

/// Initialize the media backend based on configuration
///
/// This function is idempotent - calling it multiple times will return
/// the same result.
///
/// # Arguments
/// * `requested` - The requested media backend
///
/// # Returns
/// * `MediaInitResult` describing what was actually initialized
pub fn init_media_backend(requested: MediaBackend) -> MediaInitResult {
    unsafe {
        INIT.call_once(|| {
            INIT_RESULT = Some(do_init_media_backend(requested));
        });
        INIT_RESULT.clone().expect("Media backend initialization failed")
    }
}

fn do_init_media_backend(requested: MediaBackend) -> MediaInitResult {
    match requested {
        MediaBackend::Dummy => {
            init_dummy_backend();
            MediaInitResult {
                backend: MediaBackendType::Dummy,
                message: "Dummy media backend initialized (no audio/video playback)".to_string(),
            }
        }
        MediaBackend::GStreamer => {
            match try_init_gstreamer() {
                Ok(()) => MediaInitResult {
                    backend: MediaBackendType::GStreamer,
                    message: "GStreamer media backend initialized".to_string(),
                },
                Err(e) => {
                    log::error!("GStreamer initialization failed (required): {}", e);
                    panic!("GStreamer backend requested but unavailable: {}", e);
                }
            }
        }
        MediaBackend::Auto => {
            // Try GStreamer first, fall back to dummy
            match try_init_gstreamer() {
                Ok(()) => {
                    log::info!("GStreamer media backend initialized");
                    MediaInitResult {
                        backend: MediaBackendType::GStreamer,
                        message: "GStreamer media backend initialized".to_string(),
                    }
                }
                Err(e) => {
                    log::warn!("GStreamer unavailable ({}), falling back to dummy backend", e);
                    init_dummy_backend();
                    MediaInitResult {
                        backend: MediaBackendType::Dummy,
                        message: format!(
                            "Dummy media backend initialized (GStreamer unavailable: {})",
                            e
                        ),
                    }
                }
            }
        }
    }
}

/// Initialize the dummy media backend
fn init_dummy_backend() {
    servo_media::ServoMedia::init::<servo_media_dummy::DummyBackend>();
    log::info!("Initialized dummy media backend");
}

/// Attempt to initialize GStreamer backend
#[cfg(feature = "media-gstreamer")]
fn try_init_gstreamer() -> Result<(), String> {
    use std::panic;

    // Catch panics during GStreamer initialization
    let result = panic::catch_unwind(|| {
        servo_media::ServoMedia::init::<servo_media_gstreamer::GStreamerBackend>();
    });

    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic during GStreamer initialization".to_string()
            };
            Err(msg)
        }
    }
}

/// Stub when GStreamer feature is disabled
#[cfg(not(feature = "media-gstreamer"))]
fn try_init_gstreamer() -> Result<(), String> {
    Err("GStreamer feature not compiled in".to_string())
}

/// Media playback capabilities
#[derive(Debug, Clone)]
pub struct MediaCapabilities {
    /// Can play audio
    pub audio: bool,
    /// Can play video
    pub video: bool,
    /// Supported audio codecs
    pub audio_codecs: Vec<String>,
    /// Supported video codecs
    pub video_codecs: Vec<String>,
}

impl MediaCapabilities {
    /// Get capabilities for the current backend
    pub fn current() -> Self {
        unsafe {
            match &INIT_RESULT {
                Some(result) => match result.backend {
                    MediaBackendType::GStreamer => Self::gstreamer_capabilities(),
                    MediaBackendType::Dummy => Self::dummy_capabilities(),
                },
                None => Self::dummy_capabilities(),
            }
        }
    }

    fn dummy_capabilities() -> Self {
        Self {
            audio: false,
            video: false,
            audio_codecs: vec![],
            video_codecs: vec![],
        }
    }

    fn gstreamer_capabilities() -> Self {
        // TODO: Query GStreamer for actual codec support
        Self {
            audio: true,
            video: true,
            audio_codecs: vec![
                "audio/mpeg".to_string(),   // MP3
                "audio/aac".to_string(),    // AAC
                "audio/ogg".to_string(),    // Vorbis
                "audio/opus".to_string(),   // Opus
                "audio/wav".to_string(),    // WAV
            ],
            video_codecs: vec![
                "video/h264".to_string(),   // H.264
                "video/vp8".to_string(),    // VP8
                "video/vp9".to_string(),    // VP9
                "video/av1".to_string(),    // AV1
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_backend_default() {
        assert_eq!(MediaBackend::default(), MediaBackend::Auto);
    }

    #[test]
    fn test_dummy_capabilities() {
        let caps = MediaCapabilities::dummy_capabilities();
        assert!(!caps.audio);
        assert!(!caps.video);
        assert!(caps.audio_codecs.is_empty());
    }
}
