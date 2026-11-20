//! The `frame_begin` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// A frame begins (see `FrameBegin`): before anything of it is drawn and before its world
    /// views are asked for, after the previous frame's `frame_submitted` returned.
    #[notes = r#"## Frame begin

`frame_begin` runs on the render thread at the start of every frame, before anything of it is drawn and before
`world_views` asks for the frame's extra views. It comes after the previous frame's `frame_submitted` returned, so a
mod sees frames strictly one after another:

- `device`, `queue`: the game's `wgpu::Device` and `wgpu::Queue`, as `device_ready` gave them.
- `frame`: the frame's number, counted from 1; its `frame_submitted` carries the same number.

A mod that paces frames by another clock blocks here: an OpenXR mod calls `xrWaitFrame` and `xrBeginFrame`, locates
the eyes for the predicted display time, acquires and waits for its swapchain images, and publishes the eye poses
and fields of view for its `world_views` to push this frame. Queue work (OpenXR's acquire, with a Vulkan graphics
binding) only inside `vulkan::with_raw_queue(queue, ..)`. Blocking here holds the whole frame: the window's own
present still follows its VSync setting.

The renderer calls `frame_begin` and `frame_submitted` only while a mixin replaces one of them; until then frames
run exactly as without mods. (`render_frame_pre` and `render_frame_post` are the portable, game-side frame events;
these two are the renderer's own, tied to the frame's GPU submission.)"#]
    #[hook(
        side = "client",
        contract = "listen",
        target = "mod_render::render::frame_begin",
        neoforge = "none (Horizon-only; NeoForge has no counterpart)"
    )]
    fn frame_begin();
}
