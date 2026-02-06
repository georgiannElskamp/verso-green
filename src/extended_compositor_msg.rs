use base::id::PipelineId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use ipc_channel::ipc::IpcSharedMemory;
use webrender_api::{FontInstanceFlags, FontInstanceKey, FontKey, ImageData, ImageDescriptor, ImageKey};

/// Extended compositor messages that include PipelineId for resource tracking.
/// These are used internally by Verso to track resources for cleanup.
#[derive(Debug, Serialize, Deserialize)]
pub enum ExtendedCompositorMsg {
    /// Add a font with associated pipeline tracking
    AddFont {
        /// The font key to add
        font_key: FontKey,
        /// Font index within the data
        index: u32,
        /// Font data
        data: Arc<IpcSharedMemory>,
        /// Pipeline ID for resource tracking
        pipeline_id: PipelineId,
    },
    /// Add a font instance with associated pipeline tracking
    AddFontInstance {
        /// The font instance key to add
        instance_key: FontInstanceKey,
        /// The font key this instance references
        font_key: FontKey,
        /// Font size
        size: f32,
        /// Font instance flags
        flags: FontInstanceFlags,
        /// Pipeline ID for resource tracking
        pipeline_id: PipelineId,
    },
    /// Add an image with associated pipeline tracking  
    AddImage {
        /// The image key to add
        key: ImageKey,
        /// Image descriptor
        desc: ImageDescriptor,
        /// Image data
        data: ImageData,
        /// Pipeline ID for resource tracking
        pipeline_id: PipelineId,
    },
}

impl ExtendedCompositorMsg {
    /// Get the pipeline ID associated with this message
    pub fn pipeline_id(&self) -> PipelineId {
        match self {
            ExtendedCompositorMsg::AddFont { pipeline_id, .. } => *pipeline_id,
            ExtendedCompositorMsg::AddFontInstance { pipeline_id, .. } => *pipeline_id,
            ExtendedCompositorMsg::AddImage { pipeline_id, .. } => *pipeline_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use webrender_api::{FontKey, FontInstanceKey, ImageKey};

    #[test]
    fn test_extended_msg_pipeline_id_extraction() {
        let pipeline_id = PipelineId::new(1);
        
        let font_msg = ExtendedCompositorMsg::AddFont {
            font_key: FontKey::new(1, 0),
            index: 0,
            data: Arc::new(IpcSharedMemory::from_bytes(&[]).unwrap()),
            pipeline_id,
        };
        assert_eq!(font_msg.pipeline_id(), pipeline_id);

        let instance_msg = ExtendedCompositorMsg::AddFontInstance {
            instance_key: FontInstanceKey::new(2, 0),
            font_key: FontKey::new(1, 0),
            size: 12.0,
            flags: FontInstanceFlags::empty(),
            pipeline_id,
        };
        assert_eq!(instance_msg.pipeline_id(), pipeline_id);

        let image_msg = ExtendedCompositorMsg::AddImage {
            key: ImageKey::new(3, 0),
            desc: ImageDescriptor::new(100, 100, webrender_api::ImageFormat::RGBA8, None, None, None),
            data: ImageData::new_shared(std::sync::Arc::new(vec![])),
            pipeline_id,
        };
        assert_eq!(image_msg.pipeline_id(), pipeline_id);
    }
}
