//! The `device_ready` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// The game's device just opened (see `DeviceReady`): once at startup and again each time the
    /// renderer opens a new one, before the first frame on it, on every backend. Keep clones of
    /// `device` and `queue` for work outside a render hook.
    #[notes = r#"## Device ready

A mod that works with the game's GPU outside a render hook (an OpenXR session, a capture or streaming layer, its own
worker uploading textures) needs the device the game renders with. `device_ready` hands it over once the device is
open and the window surface is set up on it, before the first frame:

- `adapter`: the `wgpu::Adapter` it was opened from (`get_info()` names the GPU and backend).
- `device`, `queue`: the `wgpu::Device` and `wgpu::Queue`. Both are cheap handles: clone and keep them.

It comes again whenever the renderer opens another device (the old one was lost, or the game fell back to another
backend); drop what you kept of the old one then. On Vulkan, `vulkan::handles(device)` gives the raw objects, for
example for `xrCreateSession` with `XR_KHR_vulkan_enable`; use the raw queue only inside
`vulkan::with_raw_queue(queue, ..)`. It is called on the thread that opens the renderer: keep it short, and do no
blocking I/O there."#]
    #[hook(
        side = "client",
        contract = "listen",
        target = "mod_render::render::device_ready",
        neoforge = "none (Horizon-only; NeoForge has no counterpart)"
    )]
    fn device_ready();
}
