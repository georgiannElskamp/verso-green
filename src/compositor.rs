use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base::cross_process_instant::CrossProcessInstant;
use base::id::{PainterId, PipelineId, WebViewId};
use base::Epoch;
use paint_api::display_list::{PaintDisplayListInfo, ScrollTree};
use paint_api::{
    CompositionPipeline, PaintMessage, PaintProxy, ImageUpdate, SendableFrameTree,
};
use constellation_traits::{
    EmbedderToConstellationMessage, PaintMetricEvent,     WindowSizeType,
};
use crossbeam_channel::{Receiver, Sender};
use dpi::PhysicalSize;
use embedder_traits::{
    AnimationState, PaintHitTestResult, Cursor, InputEvent, MouseButton, MouseButtonAction,
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
    ExternalScrollId, FontInstanceFlags, FontInstanceKey, FontInstanceOptions, FontKey, HitTestFlags,
    ImageKey, PipelineId as WebRenderPipelineId, PropertyBinding, ReferenceFrameKind, RenderReasons,
    ImageDescriptor, ImageData,
    SampledScrollOffset, ScrollLocation, SpaceAndClipInfo, SpatialId, SpatialTreeItemKey,
    TransformStyle,
};
use winit::window::WindowId;

use crate::rendering::RenderingContext;
use crate::touch::{TouchAction, TouchHandler};\
use crate::window::Window;
use crate::scroll_coalescing::ScrollCoalescer;

/// Data used to construct a compositor.
pub struct InitialCompositorState {
    /// A channel to the compositor.
    pub sender: PaintProxy,
    /// A port on which messages inbound to the compositor can be received.
    pub receiver: Receiver<PaintMessage>,
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
    compositor_receiver: Receiver<PaintMessage>,
    /// Tracks each webview and its current pipeline
    webviews: HashMap<WebViewId, PipelineId>,
    /// Tracks details about each active pipeline that the compositor knows about.
    pipeline_details: HashMap<PipelineId, PipelineDetails>,
    /// Tracks resources (fonts, images) by PainterId for proper cleanup.
    painter_resources: HashMap<PainterId, PainterResources>,
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
    /// Scroll event coalescer for batching rapid scroll events
    scroll_coalescer: ScrollCoalescer,
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

#[derive(Default)]
struct PainterResources {
    /// Track fonts associated with this painter.
    font_keys: Vec<FontKey>,
    /// Track font instances associated with this painter.
    font_instance_keys: Vec<FontInstanceKey>,
    /// Track images associated with this painter.
    image_keys: Vec<ImageKey>,
}

// Trait to allow mocking Transaction for testing
#[cfg_attr(test, mockall::automock)]
pub trait TransactionTrait {
    fn delete_font(&mut self, key: FontKey);
    fn delete_font_instance(&mut self, key: FontInstanceKey);
    fn delete_image(&mut self, key: ImageKey);
}

// Wrapper for webrender::Transaction to implement the trait
pub struct TransactionWrapper<'a>(&'a mut Transaction);

impl<'a> TransactionTrait for TransactionWrapper<'a> {
    fn delete_font(&mut self, key: FontKey) {
        self.0.delete_font(key);
    }
    fn delete_font_instance(&mut self, key: FontInstanceKey) {
        self.0.delete_font_instance(key);
    }
    fn delete_image(&mut self, key: ImageKey) {
        self.0.delete_image(key);
    }
}

impl PainterResources {
    fn add_font(&mut self, key: FontKey) {
        self.font_keys.push(key);
    }
    fn add_font_instance(&mut self, key: FontInstanceKey) {
        self.font_instance_keys.push(key);
    }
    fn add_image(&mut self, key: ImageKey) {
        self.image_keys.push(key);
    }
    fn clear(&self, txn: &mut dyn TransactionTrait) {
        for key in &self.font_keys {
            txn.delete_font(*key);
        }
        for key in &self.font_instance_keys {
            txn.delete_font_instance(*key);
        }
        for key in &self.image_keys {
            txn.delete_image(*key);
        }
    }
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
    /// The compositor-side [ScrollTree]. This is used to allow finding and scrolling
    /// nodes in the compositor before forwarding new offsets to WebRender.
    scroll_tree: ScrollTree,
    /// The paint metric status of the first paint.
    pub first_paint_metric: PaintMetricState,
    /// The paint metric status of the first contentful paint.
    pub first_contentful_paint_metric: PaintMetricState,
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
            scroll_tree: ScrollTree::default(),
            first_paint_metric: PaintMetricState::Waiting,
            first_contentful_paint_metric: PaintMetricState::Waiting,
        }
    }

    fn install_new_scroll_tree(&mut self, new_scroll_tree: ScrollTree) {
        let old_scroll_offsets: HashMap<ExternalScrollId, LayoutVector2D> = self
            .scroll_tree
            .nodes
            .drain(..)
            .filter_map(|node| match (node.external_id(), node.offset()) {
                (Some(external_id), Some(offset)) => Some((external_id, offset)),
                _ => None,
            })
            .collect();

        self.scroll_tree = new_scroll_tree;
        for node in self.scroll_tree.nodes.iter_mut() {
            match node.external_id() {
                Some(external_id) => match old_scroll_offsets.get(&external_id) {
                    Some(new_offset) => node.set_offset(*new_offset),
                    None => continue,
                },
                _ => continue,
            };
        }
    }
}

impl IOCompositor {
    /// Create a new compositor.
    pub fn new(
        current_window: WindowId,
        viewport: DeviceSize,
        scale_factor: Scale<f32, DeviceIndependentPixel, DevicePixel>,
        state: InitialCompositorState,
        wait_for_stable_image: bool,
        convert_mouse_to_touch: bool,
    ) -> Self {
        let compositor = IOCompositor {
            current_window,
            viewport,
            compositor_receiver: state.receiver,
            webviews: HashMap::new(),
            pipeline_details: HashMap::new(),
            painter_resources: HashMap::new(),
            scale_factor,
            composition_request: CompositionRequest::NoCompositingNecessary,
            touch_handler: TouchHandler::new(),
            pending_scroll_zoom_events: Vec::new(),
            shutdown_state: ShutdownState::NotShuttingDown,
            frame_tree_id: FrameTreeId(0),
            constellation_chan: state.constellation_chan,
            scroll_coalescer: ScrollCoalescer::default(),
            time_profiler_chan: state.time_profiler_chan,
            ready_to_save_state: ReadyState::Unknown,
            webrender: Some(state.webrender),
            webrender_document: state.webrender_document,
            webrender_api: state.webrender_api,
            rendering_context: state.rendering_context,
            webrender_gl: state.webrender_gl,
            cursor: Cursor::None,
            cursor_pos: DevicePoint::new(0.0, 0.0),
            wait_for_stable_image,
            convert_mouse_to_touch,
            pending_frames: 0,
            last_animation_tick: Instant::now(),
            is_animating: false,
            ready_to_present: false,
        };

        // Make sure the GL state is OK
        compositor.assert_gl_framebuffer_complete();
        compositor
    }

    /// Consume compositor itself and deinit webrender.
    pub fn deinit(&mut self) {
        if let Some(webrender) = self.webrender.take() {
            webrender.deinit();
        }
    }

    /// Get the current size of the rendering context.
    pub fn rendering_context_size(&self) -> Size2D<u32, DevicePixel> {
        self.rendering_context.size2d()
    }

    pub(crate) fn update_cursor(&mut self, pos: DevicePoint, result: &PaintHitTestResult) {
        self.cursor_pos = pos;
        // TODO: result.cursor was removed upstream. Cursor updates need to be
        // sourced from a different mechanism in the new API.
    }

    /// Tell compositor to start shutting down.
    pub fn maybe_start_shutting_down(&mut self) {
        if self.shutdown_state == ShutdownState::NotShuttingDown {
            debug!("Shutting down the constellation for WindowEvent::Quit");
            self.start_shutting_down();
        }
    }

    fn start_shutting_down(&mut self) {
        debug!("Compositor sending Exit message to Constellation");
        if let Err(e) = self
            .constellation_chan
            .send(EmbedderToConstellationMessage::Exit)
        {
            warn!("Sending exit message to constellation failed ({:?}).", e);
        }
        self.shutdown_state = ShutdownState::ShuttingDown;
        self.finish_shutting_down();
    }

    fn finish_shutting_down(&mut self) {
        debug!("Compositor received message that constellation shutdown is complete");
        // Drain compositor port, sometimes messages contain channels that are blocking
        // another thread from finishing (i.e. SetFrameTree).
        while self.compositor_receiver.try_recv().is_ok() {}

        // Tell the profiler, memory profiler, and scrolling timer to shut down.
        if let Ok((sender, receiver)) = ipc::channel() {
            self.time_profiler_chan
                .send(profile_time::ProfilerMsg::Exit(sender));
            let _ = receiver.recv();
        }

        self.shutdown_state = ShutdownState::FinishedShuttingDown;
    }

    fn handle_browser_message(
        &mut self,
        msg: PaintMessage,
        windows: &mut HashMap<WindowId, (Window, DocumentId)>,
    ) -> bool {
        match self.shutdown_state {
            ShutdownState::NotShuttingDown => {}
            ShutdownState::ShuttingDown => {
                return self.handle_browser_message_while_shutting_down(msg);
            }
            ShutdownState::FinishedShuttingDown => {
                error!("compositor shouldn't be handling messages after shutting down");
                return false;
            }
        }

        match msg {
            PaintMessage::CollectMemoryReport(sender) => {
                let ops =
                    wr_malloc_size_of::MallocSizeOfOps::new(servo_allocator::usable_size, None);
                let report = self.webrender_api.report_memory(ops);
                let reports = vec![
                    Report {
                        path: path!["webrender", "fonts"],
                        kind: ReportKind::ExplicitJemallocHeapSize,
                        size: report.fonts,
                    },
                    Report {
                        path: path!["webrender", "images"],
                        kind: ReportKind::ExplicitJemallocHeapSize,
                        size: report.images,
                    },
                    Report {
                        path: path!["webrender", "display-list"],
                        kind: ReportKind::ExplicitJemallocHeapSize,
                        size: report.display_list,
                    },
                ];
                sender.send(ProcessReports::new(reports));
            }

            PaintMessage::ChangeRunningAnimationsState(
                _webview_id,
                pipeline_id,
                animation_state,
            ) => {
                self.change_running_animations_state(pipeline_id, animation_state);
            }

            PaintMessage::CreateOrUpdateWebView(frame_tree) => {
                self.create_or_update_webview(&frame_tree, windows);
                self.send_scroll_positions_to_layout_for_pipeline(&frame_tree.pipeline.id);
            }

            PaintMessage::RemoveWebView(webview_id) => {
                self.remove_webview(webview_id, windows);
            }

            PaintMessage::SetThrottled(_webview_id, pipeline_id, throttled) => {
                self.pipeline_details(pipeline_id).throttled = throttled;
                self.process_animations(true);
            }

            PaintMessage::PipelineExited(_webview_id, pipeline_id, sender) => {
                debug!("Compositor got pipeline exited: {:?}", pipeline_id);
                self.remove_pipeline_root_layer(pipeline_id);
                let _ = sender.send(());
            }

            PaintMessage::NewWebRenderFrameReady(painter_id, _document_id, recomposite_needed) => {
                self.pending_frames -= 1;
                if recomposite_needed {
                    if let Some(result) = self.hit_test_at_point(self.cursor_pos) {
                        self.update_cursor(self.cursor_pos, &result);
                    }
                }
                if recomposite_needed || self.animation_callbacks_active() {
                    self.composite_if_necessary(CompositingReason::NewWebRenderFrame)
                }
            }

            PaintMessage::LoadComplete(_) => {
                // If we're painting in headless mode, schedule a recomposite.
                if self.wait_for_stable_image {
                    self.composite_if_necessary(CompositingReason::Headless);
                }
            }

            PaintMessage::SendInitialTransaction(_webview_id, pipeline) => {
                let mut txn = Transaction::new();
                txn.set_display_list(WebRenderEpoch(0), (pipeline, Default::default()));
                self.generate_frame(&mut txn, RenderReasons::SCENE);
                self.webrender_api
                    .send_transaction(self.webrender_document, txn);
            }

            PaintMessage::SendDisplayList {
                webview_id: _,
                display_list_descriptor,
                display_list_info_receiver,
                display_list_data_receiver,
            } => {
                // This must match the order from the sender, currently in `shared/script/lib.rs`.
                let display_list_info = match display_list_info_receiver.recv() {
                    Ok(display_list_info) => display_list_info,
                    Err(error) => {
                        // TODO: remove return true after we adapt to api based embder
                        warn!("Could not receive display list info: {error}");
                        return true;
                    }
                };
                let display_list_info: PaintDisplayListInfo =
                    match bincode::deserialize(&display_list_info) {
                        Ok(display_list_info) => display_list_info,
                        Err(error) => {
                            // TODO: remove return true after we adapt to api based embder
                            warn!("Could not deserialize display list info: {error}");
                            return true;
                        }
                    };
                let items_data = match display_list_data_receiver.recv() {
                    Ok(display_list_data) => display_list_data,
                    Err(error) => {
                        // TODO: remove return true after we adapt to api based embder
                        warn!("Could not receive WebRender display list items data: {error}");
                        return true;
                    }
                };
                let cache_data = match display_list_data_receiver.recv() {
                    Ok(display_list_data) => display_list_data,
                    Err(error) => {
                        // TODO: remove return true after we adapt to api based embder
                        warn!("Could not receive WebRender display list cache data: {error}");
                        return true;
                    }
                };
                let spatial_tree = match display_list_data_receiver.recv() {
                    Ok(display_list_data) => display_list_data,
                    Err(error) => {
                        // TODO: remove return true after we adapt to api based embder
                        warn!("Could not receive WebRender display list spatial tree: {error}.");
                        return true;
                    }
                };
                let built_display_list = BuiltDisplayList::from_data(
                    DisplayListPayload {
                        items_data,
                        cache_data,
                        spatial_tree,
                    },
                    display_list_descriptor,
                );
                let pipeline_id = display_list_info.pipeline_id;
                let details = self.pipeline_details(pipeline_id.into());
                details.most_recent_display_list_epoch = Some(display_list_info.epoch);
                details.install_new_scroll_tree(display_list_info.scroll_tree);

                let epoch = display_list_info.epoch;
                let first_reflow = display_list_info.first_reflow;
                if details.first_paint_metric == PaintMetricState::Waiting {
                    details.first_paint_metric = PaintMetricState::Seen(epoch, first_reflow);
                }
                if details.first_contentful_paint_metric == PaintMetricState::Waiting
                    && display_list_info.is_contentful
                {
                    details.first_contentful_paint_metric =
                        PaintMetricState::Seen(epoch, first_reflow);
                }

                let mut transaction = Transaction::new();
                transaction
                    .set_display_list(display_list_info.epoch, (pipeline_id, built_display_list));
                self.update_transaction_with_all_scroll_offsets(&mut transaction);
                self.generate_frame(&mut transaction, RenderReasons::SCENE);
                self.webrender_api
                    .send_transaction(self.webrender_document, transaction);
            }

            PaintMessage::GenerateImageKey(_webview_id, sender) => {
                let _ = sender.send(self.webrender_api.generate_image_key());
            }

            PaintMessage::UpdateImages(painter_id, updates) => {
                let mut txn = Transaction::new();
                for update in updates {
                    match update {
                        ImageUpdate::AddImage(key, desc, data, tiling) => {
                            txn.add_image(key, desc, data.into(), tiling);
                            self.painter_resources(painter_id).add_image(key);
                        }
                        ImageUpdate::DeleteImage(key) => txn.delete_image(key),
                        ImageUpdate::UpdateImage(key, desc, data, dirty_rect) => {
                            txn.update_image(key, desc, data.into(), &dirty_rect)
                        }
                    }
                }
                self.webrender_api
                    .send_transaction(self.webrender_document, txn);
            }

            PaintMessage::AddFont(painter_id, font_key, data, index) => {
                
                self.add_font(font_key, index, data, painter_id);
            }

            PaintMessage::AddSystemFont(painter_id, font_key, native_handle) => {
                self.painter_resources(painter_id).add_font(font_key);
                let mut transaction = Transaction::new();
                transaction.add_native_font(font_key, native_handle);
                self.webrender_api
                    .send_transaction(self.webrender_document, transaction);
            }

            PaintMessage::AddFontInstance(painter_id, font_instance_key, font_key, size, flags, _variations) => {
                
                self.add_font_instance(font_instance_key, font_key, size, flags, painter_id);
            }

            PaintMessage::RemoveFonts(painter_id, keys, instance_keys) => {
                // Remove from tracked resources
                if let Some(resources) = self.painter_resources.get_mut(&painter_id) {
                    resources.font_keys.retain(|k| !keys.contains(k));
                    resources.font_instance_keys.retain(|k| !instance_keys.contains(k));
                }
                let mut transaction = Transaction::new();
                for instance in instance_keys.into_iter() {
                    transaction.delete_font_instance(instance);
                }
                for key in keys.into_iter() {
                    transaction.delete_font(key);
                }
                self.webrender_api
                    .send_transaction(self.webrender_document, transaction);
            }

            PaintMessage::GenerateFontKeys(
                number_of_font_keys,
                number_of_font_instance_keys,
                result_sender,
                painter_id,
            ) => {
                let font_keys = (0..number_of_font_keys)
                    .map(|_| self.webrender_api.generate_font_key())
                    .collect();
                let font_instance_keys = (0..number_of_font_instance_keys)
                    .map(|_| self.webrender_api.generate_font_instance_key())
                    .collect();
                let _ = result_sender.send((font_keys, font_instance_keys));
            }
        }
        true
    }

    fn handle_browser_message_while_shutting_down(&mut self, msg: PaintMessage) -> bool {
        match msg {
            PaintMessage::PipelineExited(_webview_id, pipeline_id, sender) => {
                debug!("Compositor got pipeline exited: {:?}", pipeline_id);
                self.remove_pipeline_root_layer(pipeline_id);
                let _ = sender.send(());
            }
            PaintMessage::GenerateImageKey(_webview_id, sender) => {
                let _ = sender.send(self.webrender_api.generate_image_key());
            }
            PaintMessage::GenerateFontKeys(
                number_of_font_keys,
                number_of_font_instance_keys,
                result_sender,
                painter_id,
            ) => {
                let font_keys = (0..number_of_font_keys)
                    .map(|_| self.webrender_api.generate_font_key())
                    .collect();
                let font_instance_keys = (0..number_of_font_instance_keys)
                    .map(|_| self.webrender_api.generate_font_instance_key())
                    .collect();
                let _ = result_sender.send((font_keys, font_instance_keys));
            }
            PaintMessage::NewWebRenderFrameReady(..) => {
                // Subtract from the number of pending frames, but do not do any compositing.
                self.pending_frames -= 1;
            }
            _ => {
                debug!("Ignoring message ({:?} while shutting down", msg);
            }
        }
        true
    }

    /// Queue a new frame in the transaction and increase the pending frames count.
    fn generate_frame(&mut self, transaction: &mut Transaction, reason: RenderReasons) {
        self.pending_frames += 1;
        transaction.generate_frame(0, true /* present */, reason);
    }

    fn change_running_animations_state(
        &mut self,
        pipeline_id: PipelineId,
        animation_state: AnimationState,
    ) {
        match animation_state {
            AnimationState::AnimationsPresent => {
                let throttled = self.pipeline_details(pipeline_id).throttled;
                self.pipeline_details(pipeline_id).animations_running = true;
                if !throttled {
                    self.composite_if_necessary(CompositingReason::Animation);
                }
            }
            AnimationState::AnimationCallbacksPresent => {
                let throttled = self.pipeline_details(pipeline_id).throttled;
                self.pipeline_details(pipeline_id)
                    .animation_callbacks_running = true;
                if !throttled {
                    self.tick_animations_for_pipeline(pipeline_id);
                }
            }
            AnimationState::NoAnimationsPresent => {
                self.pipeline_details(pipeline_id).animations_running = false;
            }
            AnimationState::NoAnimationCallbacksPresent => {
                self.pipeline_details(pipeline_id)
                    .animation_callbacks_running = false;
            }
        }
    }

    fn pipeline_details(&mut self, pipeline_id: PipelineId) -> &mut PipelineDetails {
        self.pipeline_details
            .entry(pipeline_id)
            .or_insert_with(PipelineDetails::new);
        self.pipeline_details
            .get_mut(&pipeline_id)
            .expect("Insert then get failed!")
    }

    fn painter_resources(&mut self, painter_id: PainterId) -> &mut PainterResources {
        self.painter_resources
            .entry(painter_id)
            .or_insert_with(PainterResources::default)
    }

    pub fn send_root_pipeline_display_list(&mut self, window: &Window) {
        let mut transaction = Transaction::new();
        self.send_root_pipeline_display_list_in_transaction(&mut transaction, window);
        self.generate_frame(&mut transaction, RenderReasons::SCENE);
        self.webrender_api
            .send_transaction(self.webrender_document, transaction);
    }

    fn send_root_pipeline_display_list_in_transaction(
        &self,
        transaction: &mut Transaction,
        window: &Window,
    ) {
        let root_pipeline = WebRenderPipelineId(u64::from(self.current_window) as u32, 1);
        transaction.set_root_pipeline(root_pipeline);
        let mut builder = webrender::api::DisplayListBuilder::new(root_pipeline);
        builder.begin();

        let zoom_factor = self.device_pixels_per_page_pixel().0;
        let zoom_reference_frame = builder.push_reference_frame(
            LayoutPoint::zero(),
            SpatialId::root_reference_frame(root_pipeline),
            TransformStyle::Flat,
            PropertyBinding::Value(Transform3D::scale(zoom_factor, zoom_factor, 1.)),
            ReferenceFrameKind::Transform {
                is_2d_scale_translation: true,
                should_snap: true,
                paired_with_perspective: false,
            },
            SpatialTreeItemKey::new(0, 0),
        );

        let viewport_size = self.rendering_context.size2d().to_f32().to_untyped();
        let viewport_rect = LayoutRect::from_origin_and_size(
            LayoutPoint::zero(),
            LayoutSize::from_untyped(viewport_size),
        );
        let root_clip_id = builder.define_clip_rect(zoom_reference_frame, viewport_rect);
        let root_clip_chain_id = builder.define_clip_chain(None, [root_clip_id]);

        let should_decorate = window.panel.is_some();
        for webview in window.painting_order() {
            if let Some(pipeline_id) = self.webviews.get(&webview.webview_id) {
                let scaled_webview_rect =
                    LayoutRect::from_untyped(&(webview.rect.to_f32() / zoom_factor).to_untyped());
                let root_space_and_clip = if should_decorate {
                    let complex = ComplexClipRegion::new(
                        scaled_webview_rect,
                        BorderRadius::uniform(10.),
                        ClipMode::Clip,
                    );
                    let clip_id = builder.define_clip_rounded_rect(zoom_reference_frame, complex);
                    let clip_chain_id =
                        builder.define_clip_chain(Some(root_clip_chain_id), [clip_id]);
                    SpaceAndClipInfo {
                        spatial_id: zoom_reference_frame,
                        clip_chain_id,
                    }
                } else {
                    SpaceAndClipInfo {
                        spatial_id: zoom_reference_frame,
                        clip_chain_id: root_clip_chain_id,
                    }
                };
                builder.push_iframe(
                    scaled_webview_rect,
                    scaled_webview_rect,
                    &root_space_and_clip,
                    pipeline_id.into(),
                    true,
                );
                if should_decorate {
                    let root_space = SpaceAndClipInfo {
                        spatial_id: zoom_reference_frame,
                        clip_chain_id: root_clip_chain_id,
                    };
                    let offset = vec2(0., 0.);
                    let color = ColorF::new(0.0, 0.0, 0.0, 0.4);
                    let blur_radius = 5.0;
                    let spread_radius = 0.0;
                    let box_shadow_type = BoxShadowClipMode::Outset;
                    builder.push_box_shadow(
                        &CommonItemProperties::new(viewport_rect, root_space),
                        scaled_webview_rect,
                        offset,
                        color,
                        blur_radius,
                        spread_radius,
                        BorderRadius::uniform(10.),
                        box_shadow_type,
                    );
                }
            }
        }
        let built_display_list = builder.end();
        transaction.set_display_list(WebRenderEpoch(0), built_display_list);
        self.update_transaction_with_all_scroll_offsets(transaction);
    }

    fn update_transaction_with_all_scroll_offsets(&self, transaction: &mut Transaction) {
        for details in self.pipeline_details.values() {
            for node in details.scroll_tree.nodes.iter() {
                let (Some(offset), Some(external_id)) = (node.offset(), node.external_id()) else {
                    continue;
                };
                let offset = LayoutVector2D::new(-offset.x, -offset.y);
                transaction.set_scroll_offsets(
                    external_id,
                    vec![SampledScrollOffset {
                        offset,
                        generation: 0,
                    }],
                );
            }
        }
    }

    fn create_or_update_webview(
        &mut self,
        frame_tree: &SendableFrameTree,
        windows: &mut HashMap<WindowId, (Window, DocumentId)>,
    ) {
        let pipeline_id = frame_tree.pipeline.id;
        let webview_id = frame_tree.pipeline.webview_id;
        debug!("Verso Compositor is setting frame tree with pipeline {} for webview {}", pipeline_id, webview_id);
        if let Some(old_pipeline) = self.webviews.insert(webview_id, pipeline_id) {
            debug!("{webview_id}'s pipeline has changed from {old_pipeline} to {pipeline_id}");
        }
        if let Some((window, _)) = windows.get(&self.current_window) {
            self.send_root_pipeline_display_list(window);
        }
        self.create_or_update_pipeline_details_with_frame_tree(frame_tree, None);
        self.reset_scroll_tree_for_unattached_pipelines(frame_tree);
        self.frame_tree_id.next();
    }

    fn remove_webview(
        &mut self,
        webview_id: WebViewId,
        windows: &mut HashMap<WindowId, (Window, DocumentId)>,
    ) {
        debug!("Verso Compositor is removing webview {}", webview_id);
        let mut window_id = None;
        for (window, _) in windows.values_mut() {
            let (webview, close_window) = window.remove_webview(webview_id, self);
            if let Some(webview) = webview {
                if let Some(pipeline_id) = self.webviews.remove(&webview.webview_id) {
                    self.remove_pipeline_details_recursively(pipeline_id);
                }
                if close_window {
                    window_id = Some(window.id());
                } else {
                    self.send_root_pipeline_display_list(window);
                }
                self.frame_tree_id.next();
                break;
            }
        }
        if let Some(id) = window_id {
            windows.remove(&id);
        }
    }

    pub fn on_resize_webview_event(&mut self, webview_id: WebViewId, rect: DeviceRect) {
        self.send_window_size_message_for_top_level_browser_context(rect, webview_id);
    }

    fn send_window_size_message_for_top_level_browser_context(&self, rect: DeviceRect, webview_id: WebViewId) {
        let hidpi_scale_factor = self.device_pixels_per_page_pixel_not_including_page_zoom();
        let size = rect.size().to_f32() / hidpi_scale_factor;
        let msg = EmbedderToConstellationMessage::ChangeViewportDetails(
            webview_id,
            ViewportDetails { size, hidpi_scale_factor },
            WindowSizeType::Resize,
        );
        if let Err(e) = self.constellation_chan.send(msg) {
            warn!("Sending window resize to constellation failed ({:?}).", e);
        }
    }

    fn reset_scroll_tree_for_unattached_pipelines(&mut self, frame_tree: &SendableFrameTree) {
        fn collect_pipelines(pipelines: &mut HashSet<PipelineId>, frame_tree: &SendableFrameTree) {
            pipelines.insert(frame_tree.pipeline.id);
            for kid in &frame_tree.children {
                collect_pipelines(pipelines, kid);
            }
        }
        let mut attached_pipelines = HashSet::default();
        collect_pipelines(&mut attached_pipelines, frame_tree);
        self.pipeline_details
            .iter_mut()
            .filter(|(id, _)| !attached_pipelines.contains(id))
            .for_each(|(_, details)| {
                details.scroll_tree.nodes.iter_mut().for_each(|node| {
                    node.set_offset(LayoutVector2D::zero());
                })
            })
    }

    fn create_or_update_pipeline_details_with_frame_tree(&mut self, frame_tree: &SendableFrameTree, parent_pipeline_id: Option<PipelineId>) {
        let pipeline_id = frame_tree.pipeline.id;
        let pipeline_details = self.pipeline_details(pipeline_id);
        pipeline_details.pipeline = Some(frame_tree.pipeline.clone());
        pipeline_details.parent_pipeline_id = parent_pipeline_id;
        for kid in &frame_tree.children {
            self.create_or_update_pipeline_details_with_frame_tree(kid, Some(pipeline_id));
        }
    }

    fn remove_pipeline_details_recursively(&mut self, pipeline_id: PipelineId) {
        self.pipeline_details.remove(&pipeline_id);
        let children = self.pipeline_details.iter()
            .filter(|(_, pipeline_details)| pipeline_details.parent_pipeline_id == Some(pipeline_id))
            .map(|(&pipeline_id, _)| pipeline_id)
            .collect::<Vec<_>>();
        for kid in children {
            self.remove_pipeline_details_recursively(kid);
        }
    }

    fn remove_pipeline_root_layer(&mut self, pipeline_id: PipelineId) {
        self.remove_pipeline_details_recursively(pipeline_id);
    }

    pub fn swap_current_window(&mut self, window: &mut Window) {
        if window.id() != self.current_window {
            debug!("Verso Compositor swap current window from {:?} to {:?}", self.current_window, window.id());
            self.current_window = window.id();
            self.scale_factor = Scale::new(window.scale_factor() as f32);
            self.resize(window.size(), window);
        }
    }

    pub fn resize(&mut self, size: Size2D<f32, DevicePixel>, window: &mut Window) {
        if size.height == 0.0 || size.width == 0.0 { return; }
        self.on_resize_window_event(size, window);
        if let Some(panel) = &mut window.panel {
            let rect = DeviceRect::from_size(size);
            panel.webview.rect = rect;
            self.on_resize_webview_event(panel.webview.webview_id, rect);
        }
        let rect = DeviceRect::from_size(size);
        let show_tab_bar = window.tab_manager.count() > 1;
        let content_size = window.get_content_size(rect, show_tab_bar, window.show_bookmark);
        if let Some(tab_id) = window.tab_manager.current_tab_id() {
            let (tab_id, prompt_id) = window.tab_manager.set_size(tab_id, content_size);
            if let Some(tab_id) = tab_id { self.on_resize_webview_event(tab_id, content_size); }
            if let Some(prompt_id) = prompt_id { self.on_resize_webview_event(prompt_id, content_size); }
        }
        #[cfg(linux)]
        if let Some(webview_menu) = &mut window.webview_menu {
            let rect = DeviceRect::from_size(size);
            webview_menu.set_webview_rect(rect);
            self.on_resize_webview_event(webview_menu.webview().webview_id, rect);
        }
        self.send_root_pipeline_display_list(window);
    }

    pub fn on_resize_window_event(&mut self, new_viewport: DeviceSize, window: &Window) {
        if self.shutdown_state != ShutdownState::NotShuttingDown { return; }
        self.rendering_context.resize(&window.surface, PhysicalSize { width: new_viewport.width as u32, height: new_viewport.height as u32 });
        self.viewport = new_viewport;
        let mut transaction = Transaction::new();
        transaction.set_document_view(DeviceIntRect::from_size(self.viewport.to_i32()));
        self.webrender_api.send_transaction(self.webrender_document, transaction);
        self.composite_if_necessary(CompositingReason::Resize);
    }

    pub fn on_scale_factor_event(&mut self, scale_factor: f32, window: &Window) -> bool {
        if self.shutdown_state != ShutdownState::NotShuttingDown { return false; }
        self.scale_factor = Scale::new(scale_factor);
        self.update_after_zoom_or_hidpi_change(window);
        self.composite_if_necessary(CompositingReason::Resize);
        true
    }

    fn dispatch_input_event(&mut self, webview_id: WebViewId, event: InputEvent) {
        let Some(point) = event.point() else { return; };
        let Some(result) = self.hit_test_at_point(point) else { return; };
        self.update_cursor(point, &result);
        if let Err(error) = self.constellation_chan.send(EmbedderToConstellationMessage::ForwardInputEvent(webview_id, event.clone(), Some(result))) {
            warn!("Sending event to constellation failed ({error:?}).");
        }
        if let InputEvent::MouseButton(event) = &event {
            if event.action == MouseButtonAction::Down {
                let _ = self.constellation_chan.send(EmbedderToConstellationMessage::FocusWebView(webview_id));
            }
        }
    }

    pub fn on_input_event(&mut self, webview_id: WebViewId, event: InputEvent) {
        if self.shutdown_state != ShutdownState::NotShuttingDown { return; }
        if self.convert_mouse_to_touch {
            match event {
                InputEvent::MouseButton(event) => {
                    match event.action {
                        MouseButtonAction::Down => self.on_touch_down(webview_id, TouchEvent::new(TouchEventType::Down, TouchId(0), event.point)),
                        MouseButtonAction::Up => self.on_touch_up(webview_id, TouchEvent::new(TouchEventType::Up, TouchId(0), event.point)),
                        _ => {}
                    }
                    return;
                }
                InputEvent::MouseMove(event) => {
                    self.on_touch_move(webview_id, TouchEvent::new(TouchEventType::Move, TouchId(0), event.point));
                    return;
                }
                _ => {}
            }
        }
        self.dispatch_input_event(webview_id, event);
    }

    pub(crate) fn webview_id_from_point(&self, point: DevicePoint) -> Option<WebViewId> {
        self.hit_test_at_point(point)
            .map(|result| result.pipeline_id)
            .and_then(|pipeline_id| self.pipeline_details.get(&pipeline_id))
            .and_then(|details| details.pipeline.clone())
            .map(|pipeline| pipeline.webview_id)
    }

    fn hit_test_at_point(&self, point: DevicePoint) -> Option<PaintHitTestResult> {
        self.hit_test_at_point_with_flags_and_pipeline(point, HitTestFlags::empty(), None)
            .first()
            .cloned()
    }

    fn hit_test_at_point_with_flags_and_pipeline(
        &self,
        point: DevicePoint,
        flags: HitTestFlags,
        pipeline_id: Option<WebRenderPipelineId>,
    ) -> Vec<PaintHitTestResult> {
        let world_point = WorldPoint::from_untyped(point.to_untyped());
        let results = self.webrender_api.hit_test(self.webrender_document, pipeline_id, world_point, flags);
        results.items.iter().filter_map(|item| {
            let pipeline_id = item.pipeline.into();
            let details = match self.pipeline_details.get(&pipeline_id) {
                Some(details) => details,
                None => return None,
            };
            match details.most_recent_display_list_epoch {
                Some(epoch) if epoch.0 as u16 == item.tag.1 => {}
                _ => return None,
            }
            Some(PaintHitTestResult {
                pipeline_id,
                point_in_viewport: item.point_in_viewport.to_untyped(),
            })
        }).collect()
    }

    fn send_touch_event(&self, webview_id: WebViewId, event: TouchEvent) {
        let Some(result) = self.hit_test_at_point(event.point) else { return; };
        let event = InputEvent::Touch(event);
        if let Err(e) = self.constellation_chan.send(EmbedderToConstellationMessage::ForwardInputEvent(webview_id, event, Some(result))) {
            warn!("Sending event to constellation failed ({:?}).", e);
        }
    }

    pub fn on_touch_event(&mut self, webview_id: WebViewId, event: TouchEvent) {
        if self.shutdown_state != ShutdownState::NotShuttingDown { return; }
        match event.event_type {
            TouchEventType::Down => self.on_touch_down(webview_id, event),
            TouchEventType::Move => self.on_touch_move(webview_id, event),
            TouchEventType::Up => self.on_touch_up(webview_id, event),
            TouchEventType::Cancel => self.on_touch_cancel(webview_id, event),
        }
    }

    fn on_touch_down(&mut self, webview_id: WebViewId, event: TouchEvent) {
        self.touch_handler.on_touch_down(event.id, event.point);
        self.send_touch_event(webview_id, event);
    }

    fn on_touch_move(&mut self, webview_id: WebViewId, event: TouchEvent) {
        match self.touch_handler.on_touch_move(event.id, event.point) {
            TouchAction::Scroll(delta) => self.on_scroll_window_event(
                ScrollLocation::Delta(LayoutVector2D::from_untyped(delta.to_untyped())),
                event.point.cast(),
            ),
            TouchAction::Zoom(magnification, scroll_delta) => {
                let cursor = Point2D::new(-1, -1);
                for event in self.scroll_coalescer.flush() {
                    self.pending_scroll_zoom_events.push(ScrollZoomEvent::Scroll(event));
                }
                self.pending_scroll_zoom_events.push(ScrollZoomEvent::PinchZoom(magnification));
                self.pending_scroll_zoom_events.push(ScrollZoomEvent::Scroll(ScrollEvent {
                    scroll_location: ScrollLocation::Delta(LayoutVector2D::from_untyped(scroll_delta.to_untyped())),
                    cursor,
                    event_count: 1,
                }));
            }
            TouchAction::DispatchEvent => self.send_touch_event(webview_id, event),
            _ => {}
        }
    }

    fn on_touch_up(&mut self, webview_id: WebViewId, event: TouchEvent) {
        self.send_touch_event(webview_id, event);
        if let TouchAction::Click = self.touch_handler.on_touch_up(event.id, event.point) {
            self.simulate_mouse_click(webview_id, event.point);
        }
    }

    fn on_touch_cancel(&mut self, webview_id: WebViewId, event: TouchEvent) {
        self.touch_handler.on_touch_cancel(event.id, event.point);
        self.send_touch_event(webview_id, event);
    }

    fn simulate_mouse_click(&mut self, webview_id: WebViewId, point: DevicePoint) {
        let button = MouseButton::Left;
        self.dispatch_input_event(webview_id, InputEvent::MouseMove(MouseMoveEvent { point }));
        self.dispatch_input_event(webview_id, InputEvent::MouseButton(MouseButtonEvent { button, action: MouseButtonAction::Down, point }));
        self.dispatch_input_event(webview_id, InputEvent::MouseButton(MouseButtonEvent { button, action: MouseButtonAction::Up, point }));
    }

    pub fn on_scroll_event(&mut self, scroll_location: ScrollLocation, cursor: DeviceIntPoint, action: TouchEventType) {
        if self.shutdown_state != ShutdownState::NotShuttingDown { return; }
        match action {
            TouchEventType::Move => self.on_scroll_window_event(scroll_location, cursor),
            TouchEventType::Up | TouchEventType::Cancel => self.on_scroll_window_event(scroll_location, cursor),
            TouchEventType::Down => self.on_scroll_window_event(scroll_location, cursor),
        }
    }

    fn on_scroll_window_event(&mut self, scroll_location: ScrollLocation, cursor: DeviceIntPoint) {
        let event = ScrollEvent { scroll_location, cursor, event_count: 1 };
        if let Some(coalesced_events) = self.scroll_coalescer.add_event(event) {
            for coalesced in coalesced_events {
                self.pending_scroll_zoom_events.push(ScrollZoomEvent::Scroll(coalesced));
            }
        }
    }

    fn process_pending_scroll_events(&mut self, _window: &Window) {
        for event in self.scroll_coalescer.flush() {
            self.pending_scroll_zoom_events.push(ScrollZoomEvent::Scroll(event));
        }
        let mut combined_scroll_event: Option<ScrollEvent> = None;
        let mut _combined_magnification = 1.0;
        for scroll_event in self.pending_scroll_zoom_events.drain(..) {
            match scroll_event {
                ScrollZoomEvent::PinchZoom(magnification) => { _combined_magnification *= magnification }
                ScrollZoomEvent::Scroll(scroll_event_info) => {
                    let combined_event = match combined_scroll_event.as_mut() {
                        None => { combined_scroll_event = Some(scroll_event_info); continue; }
                        Some(combined_event) => combined_event,
                    };
                    match (combined_event.scroll_location, scroll_event_info.scroll_location) {
                        (ScrollLocation::Delta(old_delta), ScrollLocation::Delta(new_delta)) => {
                            let old_event_count = Scale::new(combined_event.event_count as f32);
                            combined_event.event_count += 1;
                            let new_event_count = Scale::new(combined_event.event_count as f32);
                            combined_event.scroll_location = ScrollLocation::Delta(
                                (old_delta * old_event_count + new_delta) / new_event_count,
                            );
                        }
                        (ScrollLocation::Start, _) | (ScrollLocation::End, _) => { break; }
                        (_, ScrollLocation::Start) | (_, ScrollLocation::End) => {
                            *combined_event = scroll_event_info;
                            break;
                        }
                    }
                }
            }
        }
        let scroll_result = combined_scroll_event.and_then(|combined_event| {
            self.scroll_node_at_device_point(combined_event.cursor.to_f32(), combined_event.scroll_location)
        });
        let mut transaction = Transaction::new();
        if let Some((pipeline_id, external_id, offset)) = scroll_result {
            let offset = LayoutVector2D::new(-offset.x, -offset.y);
            transaction.set_scroll_offsets(external_id, vec![SampledScrollOffset { offset, generation: 0 }]);
            self.send_scroll_positions_to_layout_for_pipeline(&pipeline_id);
        }
        self.generate_frame(&mut transaction, RenderReasons::APZ);
        self.webrender_api.send_transaction(self.webrender_document, transaction);
    }

    fn scroll_node_at_device_point(&mut self, cursor: DevicePoint, scroll_location: ScrollLocation) -> Option<(PipelineId, ExternalScrollId, LayoutVector2D)> {
        let scroll_location = match scroll_location {
            ScrollLocation::Delta(delta) => {
                let device_pixels_per_page = self.device_pixels_per_page_pixel();
                let scaled_delta = (Vector2D::from_untyped(delta.to_untyped()) / device_pixels_per_page).to_untyped();
                let calculated_delta = LayoutVector2D::from_untyped(scaled_delta);
                ScrollLocation::Delta(calculated_delta)
            }
            ScrollLocation::Start | ScrollLocation::End => scroll_location,
        };
        let hit_test_results = self.hit_test_at_point_with_flags_and_pipeline(cursor, HitTestFlags::FIND_ALL, None);
        let mut previous_pipeline_id = None;
        for result in hit_test_results.iter() {
            let pipeline_id = &result.pipeline_id;
            if previous_pipeline_id.replace(pipeline_id) != Some(pipeline_id) {
                let scroll_result = self.pipeline_details.get_mut(pipeline_id)?.scroll_tree.scroll_node_or_ancestor(&result.point_in_viewport, scroll_location);
                if let Some((external_id, offset)) = scroll_result {
                    return Some((*pipeline_id, external_id, offset));
                }
            }
        }
        None
    }

    fn process_animations(&mut self, force: bool) {
        if !force && (Instant::now() - self.last_animation_tick) < Duration::from_millis(16) { return; }
        self.last_animation_tick = Instant::now();
        let mut pipeline_ids = vec![];
        for (pipeline_id, pipeline_details) in &self.pipeline_details {
            if (pipeline_details.animations_running || pipeline_details.animation_callbacks_running) && !pipeline_details.throttled {
                pipeline_ids.push(*pipeline_id);
            }
        }
        self.is_animating = !pipeline_ids.is_empty();
        for pipeline_id in &pipeline_ids { self.tick_animations_for_pipeline(*pipeline_id) }
    }

    fn tick_animations_for_pipeline(&mut self, pipeline_id: PipelineId) {
        let animation_callbacks_running = self.pipeline_details(pipeline_id).animation_callbacks_running;
        let animations_running = self.pipeline_details(pipeline_id).animations_running;
        if !animation_callbacks_running && !animations_running { return; }
        let mut tick_type = AnimationTickType::empty();
        if animations_running { tick_type.insert(AnimationTickType::CSS_ANIMATIONS_AND_TRANSITIONS); }
        if animation_callbacks_running { tick_type.insert(AnimationTickType::REQUEST_ANIMATION_FRAME); }
        let msg = EmbedderToConstellationMessage::TickAnimation(pipeline_id, tick_type);
        if let Err(e) = self.constellation_chan.send(msg) { warn!("Sending tick to constellation failed ({:?}).", e); }
    }

    fn device_pixels_per_page_pixel(&self) -> Scale<f32, CSSPixel, DevicePixel> {
        self.device_pixels_per_page_pixel_not_including_page_zoom()
    }

    fn device_pixels_per_page_pixel_not_including_page_zoom(&self) -> Scale<f32, CSSPixel, DevicePixel> {
        Scale::new(self.scale_factor.get())
    }

    fn device_independent_int_size_viewport(&self) -> DeviceIndependentIntSize {
        (self.viewport.to_f32() / self.scale_factor).to_i32()
    }

    pub fn on_zoom_reset_window_event(&mut self, window: &Window) {
        if self.shutdown_state != ShutdownState::NotShuttingDown { return; }
        self.update_after_zoom_or_hidpi_change(window);
    }

    pub fn on_zoom_window_event(&mut self, _magnification: f32, window: &Window) {
        if self.shutdown_state != ShutdownState::NotShuttingDown { return; }
        self.update_after_zoom_or_hidpi_change(window);
    }

    fn update_after_zoom_or_hidpi_change(&mut self, window: &Window) {
        for webview in window.painting_order() {
            self.send_window_size_message_for_top_level_browser_context(webview.rect, webview.webview_id);
        }
        self.send_root_pipeline_display_list(window);
    }

    pub fn on_pinch_zoom_window_event(&mut self, magnification: f32) {
        if self.shutdown_state != ShutdownState::NotShuttingDown { return; }
        self.pending_scroll_zoom_events.push(ScrollZoomEvent::PinchZoom(magnification));
    }

    fn send_scroll_positions_to_layout_for_pipeline(&self, pipeline_id: &PipelineId) {
        let details = match self.pipeline_details.get(pipeline_id) {
            Some(details) => details,
            None => return,
        };
        let mut scroll_states = Vec::new();
        details.scroll_tree.nodes.iter().for_each(|node| {
            if let (Some(scroll_id), Some(scroll_offset)) = (node.external_id(), node.offset()) {
                scroll_states.push(ScrollState { scroll_id, scroll_offset });
            }
        });
        let message = EmbedderToConstellationMessage::SetScrollStates(*pipeline_id, scroll_states);
        let _ = self.constellation_chan.send(message);
    }

    fn animations_active(&self) -> bool {
        for details in self.pipeline_details.values() {
            if details.animations_running { return true; }
            if details.animation_callbacks_running { return true; }
        }
        false
    }

    fn animation_callbacks_active(&self) -> bool {
        self.pipeline_details.values().any(|details| details.animation_callbacks_running)
    }

    fn is_ready_to_paint_image_output(&mut self) -> Result<(), NotReadyToPaint> {
        match self.ready_to_save_state {
            ReadyState::Unknown => {
                let mut pipeline_epochs = HashMap::new();
                for id in self.pipeline_details.keys() {
                    if let Some(WebRenderEpoch(epoch)) = self.webrender.as_ref().and_then(|wr| wr.current_epoch(self.webrender_document, id.into())) {
                        let epoch = Epoch(epoch);
                        pipeline_epochs.insert(*id, epoch);
                    }
                }
                let msg = EmbedderToConstellationMessage::IsReadyToSaveImage(pipeline_epochs);
                if let Err(e) = self.constellation_chan.send(msg) { warn!("Sending ready to save to constellation failed ({:?}).", e); }
                self.ready_to_save_state = ReadyState::WaitingForConstellationReply;
                Err(NotReadyToPaint::JustNotifiedConstellation)
            }
            ReadyState::WaitingForConstellationReply => Err(NotReadyToPaint::WaitingOnConstellation),
            ReadyState::ReadyToSaveImage => {
                self.ready_to_save_state = ReadyState::Unknown;
                Ok(())
            }
        }
    }

    pub fn composite(&mut self, window: &Window) {
        match self.composite_specific_target(window) {
            Ok(_) => {
                if self.wait_for_stable_image {
                    println!("Shutting down the Constellation after generating an output file or exit flag specified");
                    self.start_shutting_down();
                }
            }
            Err(error) => { trace!("Unable to composite: {error:?}"); }
        }
    }

    fn composite_specific_target(&mut self, window: &Window) -> Result<(), UnableToComposite> {
        if let Err(err) = self.rendering_context.make_gl_context_current(&window.surface) {
            warn!("Failed to make GL context current: {:?}", err);
        }
        self.assert_no_gl_error();
        if let Some(webrender) = self.webrender.as_mut() { webrender.update(); }
        let wait_for_stable_image = self.wait_for_stable_image;
        if wait_for_stable_image {
            if self.animations_active() {
                self.process_animations(false);
                return Err(UnableToComposite::NotReadyToPaintImage(NotReadyToPaint::AnimationsActive));
            }
            if let Err(result) = self.is_ready_to_paint_image_output() {
                return Err(UnableToComposite::NotReadyToPaintImage(result));
            }
        }
        time_profile!(ProfilerCategory::Compositing, None, self.time_profiler_chan.clone(), || {
            trace!("Compositing");
            if let Some(webrender) = self.webrender.as_mut() {
                webrender.render(self.viewport.to_i32(), 0).ok();
            }
        });
        self.send_pending_paint_metrics_messages_after_composite();
        self.composition_request = CompositionRequest::NoCompositingNecessary;
        self.ready_to_present = true;
        self.process_animations(true);
        Ok(())
    }

    fn composite_if_necessary(&mut self, reason: CompositingReason) {
        trace!("Will schedule a composite {reason:?}. Previously was {:?}", self.composition_request);
        self.composition_request = CompositionRequest::CompositeNow(reason)
    }

    #[track_caller]
    fn assert_no_gl_error(&self) { debug_assert_eq!(self.webrender_gl.get_error(), gl::NO_ERROR); }

    #[track_caller]
    fn assert_gl_framebuffer_complete(&self) {
        debug_assert_eq!((self.webrender_gl.get_error(), self.webrender_gl.check_frame_buffer_status(gl::FRAMEBUFFER)), (gl::NO_ERROR, gl::FRAMEBUFFER_COMPLETE));
    }

    pub fn receive_messages(&mut self, windows: &mut HashMap<WindowId, (Window, DocumentId)>) -> bool {
        let mut compositor_messages = vec![];
        let mut found_recomposite_msg = false;
        while let Ok(msg) = self.compositor_receiver.try_recv() {
            match msg {
                PaintMessage::NewWebRenderFrameReady(..) if found_recomposite_msg => { self.pending_frames -= 1; }
                PaintMessage::NewWebRenderFrameReady(..) => { found_recomposite_msg = true; compositor_messages.push(msg) }
                _ => compositor_messages.push(msg),
            }
        }
        for msg in compositor_messages {
            if !self.handle_browser_message(msg, windows) { return false; }
        }
        true
    }

    pub fn perform_updates(&mut self, windows: &mut HashMap<WindowId, (Window, DocumentId)>) -> bool {
        if self.shutdown_state == ShutdownState::FinishedShuttingDown { return false; }
        if let Some((window, _)) = windows.get(&self.current_window) {
            match self.composition_request {
                CompositionRequest::NoCompositingNecessary => {}
                CompositionRequest::CompositeNow(_) => { self.composite(window); window.request_redraw(); }
            }
            if !self.pending_scroll_zoom_events.is_empty() { self.process_pending_scroll_events(window) }
        }
        self.shutdown_state != ShutdownState::FinishedShuttingDown
    }

    pub fn toggle_webrender_debug(&mut self, option: WebRenderDebugOption) {
        let Some(webrender) = self.webrender.as_mut() else { return; };
        let mut flags = webrender.get_debug_flags();
        let flag = match option {
            WebRenderDebugOption::Profiler => webrender::DebugFlags::PROFILER_DBG | webrender::DebugFlags::GPU_TIME_QUERIES | webrender::DebugFlags::GPU_SAMPLE_QUERIES,
            WebRenderDebugOption::TextureCacheDebug => webrender::DebugFlags::TEXTURE_CACHE_DBG,
            WebRenderDebugOption::RenderTargetDebug => webrender::DebugFlags::RENDER_TARGET_DBG,
        };
        flags.toggle(flag);
        webrender.set_debug_flags(flags);
        let mut txn = Transaction::new();
        self.generate_frame(&mut txn, RenderReasons::TESTING);
        self.webrender_api.send_transaction(self.webrender_document, txn);
    }

    fn add_image(&mut self, key: ImageKey, desc: ImageDescriptor, data: ImageData, painter_id: PainterId) {
        self.painter_resources(painter_id).add_image(key);
        let mut txn = Transaction::new();
        txn.add_image(key, desc, data.into(), None);
        self.webrender_api.send_transaction(self.webrender_document, txn);
    }

    fn add_font_instance(&mut self, instance_key: FontInstanceKey, font_key: FontKey, size: f32, flags: FontInstanceFlags, painter_id: PainterId) {
        self.painter_resources(painter_id).add_font_instance(instance_key);
        let mut transaction = Transaction::new();
        let font_instance_options = FontInstanceOptions { flags, ..Default::default() };
        transaction.add_font_instance(instance_key, font_key, size, Some(font_instance_options), None, Vec::new());
        self.webrender_api.send_transaction(self.webrender_document, transaction);
    }

    fn add_font(&mut self, font_key: FontKey, index: u32, data: Arc<IpcSharedMemory>, painter_id: PainterId) {
        self.painter_resources(painter_id).add_font(font_key);
        let mut transaction = Transaction::new();
        transaction.add_raw_font(font_key, (**data).into(), index);
        self.webrender_api.send_transaction(self.webrender_document, transaction);
    }

    fn send_pending_paint_metrics_messages_after_composite(&mut self) {
        let paint_time = CrossProcessInstant::now();
        let document_id = self.webrender_document;
        for (_, pipeline_id) in self.webviews.iter_mut() {
            debug_assert!(self.pipeline_details.contains_key(pipeline_id));
            let pipeline = self.pipeline_details.get_mut(pipeline_id).unwrap();
            let Some(current_epoch) = self.webrender.as_ref().and_then(|wr| wr.current_epoch(document_id, (*pipeline_id).into())) else { continue; };
            match pipeline.first_paint_metric {
                PaintMetricState::Seen(epoch, first_reflow) if epoch <= current_epoch => {
                    assert!(epoch <= current_epoch);
                    if let Err(error) = self.constellation_chan.send(EmbedderToConstellationMessage::PaintMetric(*pipeline_id, PaintMetricEvent::FirstPaint(paint_time, first_reflow))) {
                        warn!("Sending paint metric event to constellation failed ({error:?}).");
                    }
                    pipeline.first_paint_metric = PaintMetricState::Sent;
                }
                _ => {}
            }
            match pipeline.first_contentful_paint_metric {
                PaintMetricState::Seen(epoch, first_reflow) if epoch <= current_epoch => {
                    if let Err(error) = self.constellation_chan.send(EmbedderToConstellationMessage::PaintMetric(*pipeline_id, PaintMetricEvent::FirstContentfulPaint(paint_time, first_reflow))) {
                        warn!("Sending paint metric event to constellation failed ({error:?}).");
                    }
                    pipeline.first_contentful_paint_metric = PaintMetricState::Sent;
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum UnableToComposite {
    NotReadyToPaintImage(NotReadyToPaint),
}

#[derive(Debug, PartialEq)]
enum NotReadyToPaint {
    AnimationsActive,
    JustNotifiedConstellation,
    WaitingOnConstellation,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ReadyState {
    Unknown,
    WaitingForConstellationReply,
    ReadyToSaveImage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameTreeId(u32);

impl FrameTreeId {
    pub fn next(&mut self) {
        self.0 += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use webrender_api::{FontKey, FontInstanceKey, ImageKey};
    use mockall::predicate::*;

    #[test]
    fn test_painter_resources_clear() {
        let mut resources = PainterResources::default();
        let font_key = FontKey::new(1, 0);
        let instance_key = FontInstanceKey::new(2, 0);
        let image_key = ImageKey::new(3, 0);
        resources.add_font(font_key);
        resources.add_font_instance(instance_key);
        resources.add_image(image_key);
        let mut mock_txn = MockTransactionTrait::new();
        mock_txn.expect_delete_font().with(eq(font_key)).times(1).return_const(());
        mock_txn.expect_delete_font_instance().with(eq(instance_key)).times(1).return_const(());
        mock_txn.expect_delete_image().with(eq(image_key)).times(1).return_const(());
        resources.clear(&mut mock_txn);
    }

    #[test]
    fn test_painter_resources_add_tracks_keys() {
        let mut resources = PainterResources::default();
        let font_key1 = FontKey::new(1, 0);
        let font_key2 = FontKey::new(2, 0);
        let instance_key = FontInstanceKey::new(3, 0);
        let image_key = ImageKey::new(4, 0);
        resources.add_font(font_key1);
        resources.add_font(font_key2);
        resources.add_font_instance(instance_key);
        resources.add_image(image_key);
        assert_eq!(resources.font_keys.len(), 2);
        assert_eq!(resources.font_instance_keys.len(), 1);
        assert_eq!(resources.image_keys.len(), 1);
        assert!(resources.font_keys.contains(&font_key1));
        assert!(resources.font_keys.contains(&font_key2));
        assert!(resources.font_instance_keys.contains(&instance_key));
        assert!(resources.image_keys.contains(&image_key));
    }

    #[test]
    fn test_painter_resources_retain_after_remove() {
        let mut resources = PainterResources::default();
        let font_key1 = FontKey::new(1, 0);
        let font_key2 = FontKey::new(2, 0);
        let instance_key1 = FontInstanceKey::new(3, 0);
        let instance_key2 = FontInstanceKey::new(4, 0);
        resources.add_font(font_key1);
        resources.add_font(font_key2);
        resources.add_font_instance(instance_key1);
        resources.add_font_instance(instance_key2);
        let keys_to_remove = vec![font_key1];
        let instance_keys_to_remove = vec![instance_key1];
        resources.font_keys.retain(|k| !keys_to_remove.contains(k));
        resources.font_instance_keys.retain(|k| !instance_keys_to_remove.contains(k));
        assert_eq!(resources.font_keys.len(), 1);
        assert_eq!(resources.font_instance_keys.len(), 1);
        assert!(resources.font_keys.contains(&font_key2));
        assert!(resources.font_instance_keys.contains(&instance_key2));
    }
}
