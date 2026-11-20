//! The `device_closing` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// The game lets go of a device `device_ready` told of (see `DeviceClosing`): once, when the
    /// last renderer on it is dropped, after its work finished or it was lost.
    #[notes = r#"## Device closing

`device_closing` pairs with `device_ready`: it comes once for each device `device_ready` told of, when the game lets
it go. That is when the last renderer on it is dropped: the device was lost and the game opens another, the game
falls back to another backend, or the window closes. A resource reload keeps the device and tells nothing.

- `device`, `queue`: the device being let go, and its queue.

The renderer's submit thread has stopped and the device's work has finished (or the device was lost, or 5 s
passed) before it comes, so a mod can destroy what it made on the device: an OpenXR session and its swapchains
(after dropping any `wgpu::Texture` wrapped around their images), its buffers, and the clones it kept from
`device_ready`. On a lost device, wgpu calls fail harmlessly; destroy external objects anyway. The next
`device_ready` (a new device) comes after it. It runs on the thread that drops the renderer: keep it short."#]
    #[hook(
        side = "client",
        contract = "listen",
        target = "mod_render::render::device_closing",
        neoforge = "none (Horizon-only; NeoForge has no counterpart)"
    )]
    fn device_closing();
}
