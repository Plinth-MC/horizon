//! The `vulkan_device` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Asked once, before the Vulkan instance and device are created (not on other backends): add
    /// the instance and device extensions and features your mod needs to `VulkanRequest`. Each one
    /// is enabled only where the driver offers it, and a missing one is no error: read what was
    /// enabled from `vulkan::granted()`, and the device's raw handles from `vulkan::handles(device)`.
    #[notes = r#"## Vulkan device

A mod that drives Vulkan itself (an upscaler, a latency layer, an OpenXR runtime, an interop with another API) needs
extensions the game doesn't enable, and the raw objects of the game's device. Add what you need to `VulkanRequest` in
`vulkan_device`; the game asks it once, before it creates the Vulkan instance and device:

- `instance_extensions`, `device_extensions`: names such as `c"VK_KHR_push_descriptor"`. Push them; leave the others'.
- `timeline_semaphore`: wgpu turns timeline semaphores on wherever the GPU has them; asking only reports it.
- `present_id`: `VK_KHR_present_id` with its `presentId` feature on.
- `physical_device`: the GPU to render on, as `vulkan::VulkanDeviceId::Uuid(deviceUUID)` or `Luid(deviceLUID)` from
  `VkPhysicalDeviceIDProperties`. An OpenXR runtime renders on the `VkPhysicalDevice` that
  `xrGetVulkanGraphicsDeviceKHR` names; read its ids and ask for it here, since a handle does not hold across
  instances. When no Vulkan adapter is it, or it can't present to the window, a warning is logged and the game's own
  choice stands; `vulkan::handles(&device).device_ids` says which GPU the device is on.

Each is enabled only where the driver offers it. A missing one is no error: it's logged ("Vulkan device extension
"X" asked by a mod is not offered here") and left out, and the game starts as it would without it. Read what was
enabled from `vulkan::granted()` once the renderer is up. With no mod asking, the instance and device open exactly as
they do without mods, and on other backends (DirectX 12, Metal, OpenGL) nothing is asked and every `vulkan` call
returns `None`.

The raw objects come from `mod_render::render::vulkan`, which keeps the unsafe code in the game:

- `vulkan::handles(&device)`: `VkInstance`, `VkPhysicalDevice`, `VkDevice`, the queue, its family and index,
  `vkGetInstanceProcAddr` and the device's enabled extensions, as Vulkan handle values. Load the functions you call
  through `get_instance_proc_addr`.
- `vulkan::image(&texture)`: the `VkImage` and `VkFormat` of a `wgpu::Texture`; `vulkan::view(&view)` a
  `VkImageView`; `vulkan::format(format)` the `VkFormat` a `wgpu::TextureFormat` is created with.

The game owns all of them: never destroy one, and use them only while the `wgpu::Device` or texture lives. Submit to
the queue only on the render thread, from a render hook: Vulkan queues need external synchronisation, and the game
submits there too."#]
    #[hook(
        side = "client",
        contract = "collect",
        out = "request",
        target = "mod_render::render::vulkan_device",
        neoforge = "none (Horizon-only; NeoForge has no counterpart)"
    )]
    fn vulkan_device();
}
