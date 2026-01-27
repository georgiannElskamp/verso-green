use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base::cross_process_instant::CrossProcessInstant;
use base::id::{PipelineId, WebViewId};
use base::{Epoch, WebRenderEpochToU16};
use compositing_traits::display_list::{CompositorDisplayListInfo, HitTestInfo, ScrollTree};
use compositing_traits::{
    CompositionPipeline, CompositorMsg, CompositorProxy, ImageUpdate, SendableFrameTree,
};
use constellation_traits::{
    AnimationTickType, EmbedderToConstellationMessage, PaintMetricEvent, ScrollState,
    WindowSizeType,
};
use crossbeam_channel::{Receiver, Sender};
use dpi::PhysicalSize;
use embedder_traits::{
    AnimationState, CompositorHitTestResult, Cursor, InputEvent, MouseButton, MouseButtonAction,
    MouseButtonEvent, MouseMoveEvent, TouchEvent, TouchEventType, TouchId, UntrustedNodeAddress,
    ViewportDetails,
};
use euclid::{Point2D, Scale, Size2D, Transform3D, Vector2D, vec2};
use gleam::gl;
use ipc_channel::ipc::{self, IpcSharedMemory};
use log::{debug, error, trace, warn};
use profile_traits::mem::{ProcessReports, Report, ReportKind};
use profile_traits::time::{self as profile_time, ProfilerCategory};
use profile_traits::{mem, path, time, time_profile};
use servo_geometry::{DeviceIndependentIntSize, DeviceIndependentPixel};
use style_traits::CSSPixel;
use webrender::{RenderApi, Transaction};
use webrender_api::units::{
    DeviceIntPoint, DeviceIntRect, DevicePixel, DevicePoint, DeviceRect, DeviceSize, LayoutPoint,
    LayoutRect, LayoutSize, LayoutVector2D, WorldPoint,
};
use webrender_api::{
    BorderRadius, BoxShadowClipMode, BuiltDisplayList, ClipMode, ColorF, CommonItemProperties,
    ComplexClipRegion, DirtyRect, DisplayListPayload, DocumentId, Epoch as WebRenderEpoch,
    ExternalScrollId, FontInstanceFlags, FontInstanceKey, FontInstanceOptions, FontKey,
    HitTestFlags, ImageKey, PipelineId as WebRenderPipelineId, PropertyBinding, ReferenceFrameKind,
    RenderReasons, SampledScrollOffset, ScrollLocation, SpaceAndClipInfo, SpatialId,
    SpatialTreeItemKey, TransformStyle,
};
use winit::window::WindowId;

use crate::rendering::RenderingContext;
use crate::touch::{TouchAction, TouchHandler};
use crate::window::Window;

/// Data used to construct a compositor.
pub struct InitialCompositorState {
    /// A channel to the compositor.
    pub sender: CompositorProxy,
    /// A port on which messages inbound to the compositor can be received.
    pub receiver: Receiver<CompositorMsg>,
    /// A channel to the constellation.
    pub constellation_chan: Sender<EmbedderToConstellationMessage>,
    /// A channel to the time profiler thread.
    pub time_profiler_chan: time::ProfilerChan,
    /// A channel to the memory profiler thread.
    pub mem_profiler_chan: mem::ProfilerChan,
    /// Instance of webrender API
    pub webrender: webrender::Renderer,
    /// Webrender document ID
    pub webrender_document: DocumentId,
    /// Webrender API
    pub webrender_api: RenderApi,
    /// Servo's rendering context
    pub rendering_context: RenderingContext,
    /// Webrender GL handle
    pub webrender_gl: Rc<dyn gl::Gl>,
}

/// Various debug and profiling flags that WebRender supports.
#[derive(Clone)]
pub enum WebRenderDebugOption {
    /// Set profiler flags to webrender.
    Profiler,
    /// Set texture cache flags to webrender.
    TextureCacheDebug,
    /// Set render target flags to webrender.
    RenderTargetDebug,
}

/// Mouse event for the compositor.
#[derive(Clone)]
pub enum MouseWindowEvent {
    /// Mouse click event
    Click(MouseButton, DevicePoint),
    /// Mouse down event
    MouseDown(MouseButton, DevicePoint),
    /// Mouse up event
    MouseUp(MouseButton, DevicePoint),
}

// NB: Never block on the Constellation, because sometimes the Constellation blocks on us.
/// The Verso compositor contains a GL rendering context with a WebRender instance.
/// The compositor will communicate with Servo using messages from the Constellation,
/// then composite the WebRender frames and present the surface to the window.
pub struct IOCompositor {
    /// The current window that Compositor is handling.
    pub current_window: WindowId,

    /// Size of current viewport that Compositor is handling.
    viewport: DeviceSize,

    /// The pixel density of the display.
    scale_factor: Scale<f32, DeviceIndependentPixel, DevicePixel>,

    /// The active webrender document.
    webrender_document: DocumentId,

    /// The port on which we receive messages.
    compositor_receiver: Receiver<CompositorMsg>,

    /// Tracks each webview and its current pipeline
    webviews: HashMap<WebViewId, PipelineId>,

    /// Tracks details about each active pipeline that the compositor knows about.
    pipeline_details: HashMap<PipelineId, PipelineDetails>,

    /// Tracks whether we should composite this frame.
    composition_request: CompositionRequest,

    /// check if the surface is ready to present.
    pub ready_to_present: bool,

    /// Tracks whether we are in the process of shutting down, or have shut down and should close
    /// the compositor.
    pub shutdown_state: ShutdownState,

    /// The current frame tree ID (used to reject old paint buffers)
    frame_tree_id: FrameTreeId,

    /// The channel on which messages can be sent to the constellation.
    pub constellation_chan: Sender<EmbedderToConstellationMessage>,

    /// The channel on which messages can be sent to the time profiler.
    time_profiler_chan: profile_time::ProfilerChan,

    /// Touch input state machine
    touch_handler: TouchHandler,

    /// Pending scroll/zoom events.
    pending_scroll_zoom_events: Vec<ScrollZoomEvent>,

    /// Used by the logic that determines when it is safe to output an
    /// image for the reftest framework.
    ready_to_save_state: ReadyState,

    /// The webrender renderer.
    webrender: Option<webrender::Renderer>,

    /// The webrender interface, if enabled.
    pub webrender_api: RenderApi,

    /// The glutin instance that webrender targets
    pub rendering_context: RenderingContext,

    /// The GL bindings for webrender
    webrender_gl: Rc<dyn gl::Gl>,

    /// Current mouse cursor.
    cursor: Cursor,

    /// Current cursor position.
    cursor_pos: DevicePoint,

    /// True to exit after page load ('-x').
    wait_for_stable_image: bool,

    /// True to translate mouse input into touch events.
    convert_mouse_to_touch: bool,

    /// The number of frames pending to receive from WebRender.
    pending_frames: usize,

    /// The [`Instant`] of the last animation tick, used to avoid flooding the Constellation and
    /// ScriptThread with a deluge of animation ticks.
    last_animation_tick: Instant,

    /// Whether the application is currently animating.
    /// Typically, when animations are active, the window
    /// will want to avoid blocking on UI events, and just
    /// run the event loop at the vsync interval.
    pub is_animating: bool,
}

#[derive(Clone, Copy)]
struct ScrollEvent {
    /// Scroll by this offset, or to Start or End
    scroll_location: ScrollLocation,
    /// Apply changes to the frame at this location
    cursor: DeviceIntPoint,
    /// The number of OS events that have been coalesced together into this one event.
    event_count: u32,
}

#[derive(Clone, Copy)]
enum ScrollZoomEvent {
    /// An pinch zoom event that magnifies the view by the given factor.
    PinchZoom(f32),
    /// A scroll event that scrolls the scroll node at the given location by the
    /// given amount.
    Scroll(ScrollEvent),
}

/// Why we performed a composite. This is used for debugging.
///
/// TODO: It would be good to have a bit more precision here about why a composite
/// was originally triggered, but that would require tracking the reason when a
/// frame is queued in WebRender and then remembering when the frame is ready.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CompositingReason {
    /// We're performing the single composite in headless mode.
    Headless,
    /// We're performing a composite to run an animation.
    Animation,
    /// A new WebRender frame has arrived.
    NewWebRenderFrame,
    /// The window has been resized and will need to be synchronously repainted.
    Resize,
}

#[derive(Debug, PartialEq)]
enum CompositionRequest {
    NoCompositingNecessary,
    CompositeNow(CompositingReason),
}

/// Shutdown State of the compositor
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShutdownState {
    /// Compositor is still running.
    NotShuttingDown,
    /// Compositor is shutting down.
    ShuttingDown,
    /// Compositor has shut down.
    FinishedShuttingDown,
}

/// The paint status of a particular pipeline in the Servo renderer. This is used to trigger metrics
/// in script (via the constellation) when display lists are received.
///
/// See <https://w3c.github.io/paint-timing/#first-contentful-paint>.
#[derive(PartialEq)]
pub(crate) enum PaintMetricState {
    /// The renderer is still waiting to process a display list which triggers this metric.
    Waiting,
    /// The renderer has processed the display list which will trigger this event, marked the Servo
    /// instance ready to paint, and is waiting for the given epoch to actually be rendered.
    Seen(WebRenderEpoch, bool /* first_reflow */),
    /// The metric has been sent to the constellation and no more work needs to be done.
    Sent,
}

/// Resources associated with a pipeline that need to be cleaned up when the pipeline is removed.
#[derive(Default)]
struct PipelineResources {
    /// Track fonts associated with this pipeline
    font_keys: Vec<FontKey>,
    /// Track font instances
    font_instance_keys: Vec<FontInstanceKey>,
    /// Track images
    image_keys: Vec<ImageKey>,
}

struct PipelineDetails {
    /// The pipeline associated with this PipelineDetails object.
    pipeline: Option<CompositionPipeline>,

    /// The id of the parent pipeline, if any.
    parent_pipeline_id: Option<PipelineId>,

    /// The epoch of the most recent display list for this pipeline. Note that this display
    /// list might not be displayed, as WebRender processes display lists asynchronously.
    most_recent_display_list_epoch: Option<WebRenderEpoch>,

    /// Whether animations are running
    animations_running: bool,

    /// Whether there are animation callbacks
    animation_callbacks_running: bool,

    /// Whether to use less resources by stopping animations.
    throttled: bool,

    /// Hit test items for this pipeline. This is used to map WebRender hit test
    /// information to the full information necessary for Servo.
    hit_test_items: Vec<HitTestInfo>,

    /// The compositor-side [ScrollTree]. This is used to allow finding and scrolling
    /// nodes in the compositor before forwarding new offsets to WebRender.
    scroll_tree: ScrollTree,

    /// The paint metric status of the first paint.
    pub first_paint_metric: PaintMetricState,

    /// The paint metric status of the first contentful paint.
    pub first_contentful_paint_metric: PaintMetricState,

    /// Resources associated with this pipeline that need cleanup.
    resources: PipelineResources,
}

impl PipelineDetails {
    fn new() -> PipelineDetails {
        PipelineDetails {
            pipeline: None,
            parent_pipeline_id: None,
            most_recent_display_list_epoch: None,
            animations_running: false,
            animation_callbacks_running: false,
            throttled: false,
            hit_test_items: Vec::new(),
            scroll_tree: ScrollTree::default(),
            first_paint_metric: PaintMetricState::Waiting,
            first_contentful_paint_metric: PaintMetricState::Waiting,
            resources: PipelineResources::default(),
        }
    }