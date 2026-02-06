//! Resource tracking for pipeline cleanup
//!
//! This module provides resource tracking to ensure WebRender resources
//! (images, fonts, font instances) are properly cleaned up when pipelines exit.

use webrender_api::{FontInstanceKey, FontKey, ImageKey};

/// Tracks resources associated with a pipeline for cleanup
#[derive(Default, Debug, Clone)]
pub struct PipelineResources {
    /// Image keys allocated by this pipeline
    pub image_keys: Vec<ImageKey>,
    /// Font keys allocated by this pipeline
    pub font_keys: Vec<FontKey>,
    /// Font instance keys allocated by this pipeline
    pub font_instance_keys: Vec<FontInstanceKey>,
}

impl PipelineResources {
    /// Create a new empty resource tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Track a new image key
    pub fn track_image(&mut self, key: ImageKey) {
        self.image_keys.push(key);
    }

    /// Track a new font key
    pub fn track_font(&mut self, key: FontKey) {
        self.font_keys.push(key);
    }

    /// Track a new font instance key
    pub fn track_font_instance(&mut self, key: FontInstanceKey) {
        self.font_instance_keys.push(key);
    }

    /// Remove tracking for an image key (e.g., when explicitly deleted)
    pub fn untrack_image(&mut self, key: ImageKey) {
        self.image_keys.retain(|k| *k != key);
    }

    /// Remove tracking for a font key
    pub fn untrack_font(&mut self, key: FontKey) {
        self.font_keys.retain(|k| *k != key);
    }

    /// Remove tracking for a font instance key
    pub fn untrack_font_instance(&mut self, key: FontInstanceKey) {
        self.font_instance_keys.retain(|k| *k != key);
    }

    /// Check if any resources are being tracked
    pub fn is_empty(&self) -> bool {
        self.image_keys.is_empty()
            && self.font_keys.is_empty()
            && self.font_instance_keys.is_empty()
    }

    /// Get total count of tracked resources
    pub fn resource_count(&self) -> usize {
        self.image_keys.len() + self.font_keys.len() + self.font_instance_keys.len()
    }

    /// Clear all tracked resources (after cleanup transaction sent)
    pub fn clear(&mut self) {
        self.image_keys.clear();
        self.font_keys.clear();
        self.font_instance_keys.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_tracking() {
        let mut resources = PipelineResources::new();
        assert!(resources.is_empty());

        // Note: In real tests, we'd use actual WebRender key types
        // For now, this demonstrates the API
        assert_eq!(resources.resource_count(), 0);
    }

    #[test]
    fn test_clear_resources() {
        let mut resources = PipelineResources::new();
        resources.clear();
        assert!(resources.is_empty());
    }
}
