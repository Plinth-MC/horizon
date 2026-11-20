//! Engine-owned frame data and load-time render dispatch slots.
use crate::render_geometry::{EntitySpan, Vertex};
pub use crate::render_views::{
    WorldView, WorldViewDrawn, WorldViewRefusal, WorldViewRefusalReason, WorldViews,
};
use glam::{DVec3, Mat4};
/// What native mods may ask of Vulkan before the device opens, and its raw handles after.
pub use horizon_hdr::vulkan;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
pub struct WorldDrawn<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub color: &'a wgpu::Texture,
    pub color_view: &'a wgpu::TextureView,
    pub color_format: wgpu::TextureFormat,
    pub size: [u32; 2],
    pub depth: &'a wgpu::TextureView,
    pub depth_size: [u32; 2],
    pub view_proj: Mat4,
    pub jittered: Mat4,
    pub camera: DVec3,
    pub entities: &'a [Vertex],
    pub spans: &'a [EntitySpan],
    pub motion: &'a [(u32, u64)],
    pub atlas: Option<&'a wgpu::Texture>,
    pub quad_indices: &'a wgpu::Buffer,
    pub block_sprites: BlockSpriteArrays<'a>,
}
pub struct BlockSpriteArrays<'a> {
    pub views: &'a [wgpu::TextureView],
    pub split: u32,
    pub size: u32,
    pub nearest: &'a wgpu::Sampler,
    pub filtered: &'a wgpu::Sampler,
}
pub struct HandDrawn<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub color: &'a wgpu::Texture,
    pub color_view: &'a wgpu::TextureView,
    pub color_format: wgpu::TextureFormat,
    pub size: [u32; 2],
    pub hand: &'a [Vertex],
    pub view_proj: Mat4,
    pub camera: DVec3,
    pub atlas: Option<&'a wgpu::Texture>,
    pub quad_indices: &'a wgpu::Buffer,
}
/// The game's device just opened (see `device_ready`): at startup, and again each time the
/// renderer opens a new one (a lost device, a backend switch).
pub struct DeviceReady<'a> {
    pub adapter: &'a wgpu::Adapter,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
}
/// The game is letting go of a device (see `device_closing`): its last renderer is being
/// dropped, after the device's work finished (or it was lost).
pub struct DeviceClosing<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
}
/// A frame is about to be drawn (see `frame_begin`): before its world views are asked for.
pub struct FrameBegin<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    /// Counts the frames `frame_begin` was told of, from 1; [`FrameSubmitted::frame`] repeats it.
    pub frame: u64,
}
/// A frame's commands were submitted (see `frame_submitted`), before its window image is
/// presented. May come on the renderer's submit thread.
pub struct FrameSubmitted {
    /// The [`FrameBegin::frame`] it began as.
    pub frame: u64,
    /// Whether the frame was drawn and submitted; false when it was not drawn (minimized, a
    /// lost surface or device, an error), and nothing of it reached the GPU.
    pub drawn: bool,
}
#[derive(Default)]
pub struct RenderNeeds {
    pub entity_spans: bool,
    pub world_depth: bool,
}
/// Installed once by the rendering provider before startup. Dispatch never allocates.
pub struct Slots {
    pub world_views: fn(&mut WorldViews),
    pub world_views_hooked: fn() -> bool,
    pub world_view_drawn: for<'a> fn(&mut WorldViewDrawn<'a>),
    pub world_drawn: for<'a> fn(&mut WorldDrawn<'a>),
    pub hand_drawn: for<'a> fn(&mut HandDrawn<'a>),
    pub render_needs: fn(&mut RenderNeeds),
    pub world_drawn_hooked: fn() -> bool,
    /// Asked once before the Vulkan instance and device are created.
    pub vulkan_device: fn(&mut vulkan::VulkanRequest),
    /// Told once each time a device opens, before the first frame on it.
    pub device_ready: for<'a> fn(&mut DeviceReady<'a>),
    /// Told once for each device `device_ready` told of, when the game lets it go.
    pub device_closing: for<'a> fn(&mut DeviceClosing<'a>),
    /// Whether a mod listens to `frame_begin` or `frame_submitted`: while none does, neither is
    /// called and frames run as without mods.
    pub frame_hooked: fn() -> bool,
    /// Told at the start of each frame, before its world views are asked for.
    pub frame_begin: for<'a> fn(&mut FrameBegin<'a>),
    /// Told once per `frame_begin`, after the frame's submit (or when it was not drawn), before
    /// its window image is presented.
    pub frame_submitted: fn(&mut FrameSubmitted),
}
static SLOTS: OnceLock<Slots> = OnceLock::new();
pub fn install(slots: Slots) {
    let _ = SLOTS.set(slots);
}
pub fn world_drawn(world: &mut WorldDrawn<'_>) {
    if let Some(s) = SLOTS.get() {
        (s.world_drawn)(world);
    }
}
pub fn hand_drawn(hand: &mut HandDrawn<'_>) {
    if let Some(s) = SLOTS.get() {
        (s.hand_drawn)(hand);
    }
}
pub fn needs() -> RenderNeeds {
    let mut n = RenderNeeds::default();
    if let Some(s) = SLOTS.get() {
        (s.render_needs)(&mut n);
    }
    n
}
pub fn world_drawn_hooked() -> bool {
    SLOTS.get().is_some_and(|s| (s.world_drawn_hooked)())
}

/// Gather the provider's requested views, each taken or refused as it is pushed.
pub fn world_views(views: &mut WorldViews) {
    if let Some(s) = SLOTS.get() {
        (s.world_views)(views);
    }
}
pub fn world_views_hooked() -> bool {
    SLOTS.get().is_some_and(|s| (s.world_views_hooked)())
}
pub fn world_view_drawn(view: &mut WorldViewDrawn<'_>) {
    if let Some(s) = SLOTS.get() {
        (s.world_view_drawn)(view);
    }
}

/// Tell the provider's mods that the game's device opened.
pub fn device_ready(ready: &mut DeviceReady<'_>) {
    if let Some(s) = SLOTS.get() {
        (s.device_ready)(ready);
    }
}

/// Tell the provider's mods that the game lets go of a device it told them of.
pub fn device_closing(closing: &mut DeviceClosing<'_>) {
    if let Some(s) = SLOTS.get() {
        (s.device_closing)(closing);
    }
}

/// Whether the provider's mods listen to the frame's start and submission.
pub fn frame_hooked() -> bool {
    SLOTS.get().is_some_and(|s| (s.frame_hooked)())
}

static FRAMES: AtomicU64 = AtomicU64::new(0);

/// Tell the provider's mods a frame begins; the frame's number, to hand to [`frame_submitted`]
/// exactly once, or None (and no call) while no mod listens.
pub fn frame_begin(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<u64> {
    let s = SLOTS.get().filter(|s| (s.frame_hooked)())?;
    let frame = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    (s.frame_begin)(&mut FrameBegin {
        device,
        queue,
        frame,
    });
    Some(frame)
}

/// Tell the provider's mods that frame `frame` ([`frame_begin`]) was submitted, or not `drawn`.
pub fn frame_submitted(frame: u64, drawn: bool) {
    if let Some(s) = SLOTS.get() {
        (s.frame_submitted)(&mut FrameSubmitted { frame, drawn });
    }
}

/// What the provider's mods ask of Vulkan: empty without a provider, so the instance and the
/// device open as wgpu opens them.
pub fn vulkan_request() -> vulkan::VulkanRequest {
    let mut request = vulkan::VulkanRequest::default();
    if let Some(s) = SLOTS.get() {
        (s.vulkan_device)(&mut request);
    }
    request
}
