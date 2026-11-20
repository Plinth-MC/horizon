//! The `frame_submitted` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// The frame `frame_begin` began was submitted, or was not drawn (see `FrameSubmitted`):
    /// once per `frame_begin`, before the frame's window image is presented.
    #[notes = r#"## Frame submitted

`frame_submitted` comes exactly once for every `frame_begin`, with the same `frame` number:

- `drawn: true`: the frame's commands were submitted to the game's queue, and its window image is not presented
  yet. Anything the frame drew into is now on the GPU's queue in order: release swapchain images and end the frame
  here (OpenXR: `xrReleaseSwapchainImage` for each eye, then `xrEndFrame` with the projection layer).
- `drawn: false`: nothing of the frame reached the GPU (the window is minimized, the surface or device was lost, the
  frame failed). End the frame without layers.

It may run on the renderer's submit thread (Linux by default), not the render thread: keep what it shares with
`frame_begin` behind a lock. The next `frame_begin` waits until it returned. Queue work only inside
`vulkan::with_raw_queue(queue, ..)`, with the queue `device_ready` gave; never submit through wgpu there."#]
    #[hook(
        side = "client",
        contract = "listen",
        target = "mod_render::render::frame_submitted",
        neoforge = "none (Horizon-only; NeoForge has no counterpart)"
    )]
    fn frame_submitted();
}
