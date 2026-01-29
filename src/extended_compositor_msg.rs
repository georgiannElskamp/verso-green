use base::id::PipelineId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use ipc_channel::ipc::IpcSharedMemory;
use webrender_api::{FontInstanceFlags, FontInstanceKey, FontKey, ImageData, ImageDescriptor, ImageKey};

#[derive(Debug, Serialize, Deserialize)]
pub enum ExtendedCompositorMsg {
    AddFont {
        font_key: FontKey,
        index: u32,
        data: Arc<IpcSharedMemory>,
        pipeline_id: PipelineId,
    },
    AddFontInstance {
        instance_key: FontInstanceKey,
        font_key: FontKey,
        size: f32,
        flags: FontInstanceFlags,
        pipeline_id: PipelineId,
    },
    AddImage {
        key: ImageKey,
        desc: ImageDescriptor,
        data: ImageData,
        pipeline_id: PipelineId,
    },
}
