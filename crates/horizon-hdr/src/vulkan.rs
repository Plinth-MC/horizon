//! What native mods may ask of Vulkan, and the raw handles they get back.
//!
//! Before the instance and the device are created the renderer collects a
//! [`VulkanRequest`] from the mods (horizon-mod-render's `mod_render::render::vulkan_device`
//! hook). Each extension or feature in it is enabled only when the driver offers
//! it: what is missing is logged and left out, never an error. Once the device
//! exists, [`handles`], [`image`], [`view`] and [`format`] hand out its raw
//! Vulkan objects, so the unsafe code stays in this crate. Every function here is
//! a no-op on other backends; with an empty request the device opens exactly as
//! [`crate::open_vulkan_device`] (or wgpu's own `request_device`) opens it.
use std::ffi::CStr;
use std::sync::Mutex;

/// What the mods ask Vulkan to enable. Fields may be added: the struct is `#[non_exhaustive]`.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VulkanRequest {
    /// Instance extensions, such as `c"VK_KHR_get_surface_capabilities2"`.
    pub instance_extensions: Vec<&'static CStr>,
    /// Device extensions, such as `c"VK_KHR_push_descriptor"`.
    pub device_extensions: Vec<&'static CStr>,
    /// Timeline semaphores. wgpu enables them whenever the GPU has them; asking only makes
    /// [`VulkanGranted::timeline_semaphore`] say whether they are there.
    pub timeline_semaphore: bool,
    /// `VK_KHR_present_id` with its `presentId` feature turned on.
    pub present_id: bool,
    /// The GPU to render on, when a mod must have a given one: an OpenXR runtime renders on the
    /// `VkPhysicalDevice` `xrGetVulkanGraphicsDeviceKHR` names, and a device on another GPU
    /// can't share its swapchain images. Named by its `VkPhysicalDeviceIDProperties` ids, which
    /// hold across instances (a `VkPhysicalDevice` handle does not). When no Vulkan adapter has
    /// it, a warning is logged and the game's own choice stands. `None`: the game chooses.
    pub physical_device: Option<VulkanDeviceId>,
}

/// A physical device as [`VulkanRequest::physical_device`] names it.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VulkanDeviceId {
    /// `VkPhysicalDeviceIDProperties::deviceUUID`.
    Uuid([u8; 16]),
    /// `VkPhysicalDeviceIDProperties::deviceLUID` (Windows), matched only where the driver
    /// says it is valid (`deviceLUIDValid`).
    Luid([u8; 8]),
}

/// The ids a Vulkan adapter's physical device reports ([`device_ids`]).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VulkanDeviceIds {
    /// `deviceUUID`.
    pub uuid: [u8; 16],
    /// `deviceLUID`, when `deviceLUIDValid`.
    pub luid: Option<[u8; 8]>,
}

impl VulkanDeviceIds {
    /// Ids as a driver reports them, for building one outside this crate (tests, mocks).
    pub fn new(uuid: [u8; 16], luid: Option<[u8; 8]>) -> Self {
        Self { uuid, luid }
    }
}

impl VulkanDeviceId {
    /// Whether a physical device with `ids` is this one.
    pub fn matches(&self, ids: &VulkanDeviceIds) -> bool {
        match self {
            Self::Uuid(uuid) => ids.uuid == *uuid,
            Self::Luid(luid) => ids.luid == Some(*luid),
        }
    }
}

/// The first of `adapters` (their ids, `None` for one that is not Vulkan or can't say) that is
/// `want`.
pub fn pick_device(adapters: &[Option<VulkanDeviceIds>], want: &VulkanDeviceId) -> Option<usize> {
    adapters
        .iter()
        .position(|ids| ids.as_ref().is_some_and(|ids| want.matches(ids)))
}

/// The adapter of `adapters` that `request` names ([`VulkanRequest::physical_device`]), or
/// `None` when it names none or none of them is it (then it logs a warning).
pub fn wanted_adapter(adapters: &[wgpu::Adapter], request: &VulkanRequest) -> Option<usize> {
    let want = request.physical_device.as_ref()?;
    let ids: Vec<_> = adapters.iter().map(device_ids).collect();
    let found = pick_device(&ids, want);
    if found.is_none() {
        log::warn!(
            "Vulkan physical device {want:?} asked by a mod is not among this machine's {} \
             Vulkan adapter(s); the game picks its own",
            ids.iter().flatten().count()
        );
    }
    found
}

/// The ids of `adapter`'s physical device; `None` unless it is a Vulkan adapter on a Vulkan 1.1
/// instance and device.
pub fn device_ids(adapter: &wgpu::Adapter) -> Option<VulkanDeviceIds> {
    imp::device_ids(adapter)
}

impl VulkanRequest {
    /// Nothing asked for: the instance and the device open as they would with no mod.
    pub fn is_empty(&self) -> bool {
        self.instance_extensions.is_empty() && !self.asks_device() && self.physical_device.is_none()
    }

    fn asks_device(&self) -> bool {
        !self.device_extensions.is_empty() || self.timeline_semaphore || self.present_id
    }
}

/// What of a [`VulkanRequest`] the driver offered and the engine enabled.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VulkanGranted {
    /// The requested instance extensions the instance was created with.
    pub instance_extensions: Vec<&'static CStr>,
    /// The requested device extensions the device was created with.
    pub device_extensions: Vec<&'static CStr>,
    /// Timeline semaphores were asked for and the device has them.
    pub timeline_semaphore: bool,
    /// `VK_KHR_present_id` and its feature were asked for and enabled.
    pub present_id: bool,
}

/// The raw Vulkan objects of a wgpu device, as Vulkan's handle values. They are valid
/// while the `wgpu::Device` lives; the engine owns them: never destroy them.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VulkanHandles {
    /// `VkInstance`.
    pub instance: u64,
    /// `VkPhysicalDevice`.
    pub physical_device: u64,
    /// `VkDevice`.
    pub device: u64,
    /// The `VkQueue` wgpu submits on.
    pub queue: u64,
    /// That queue's family.
    pub queue_family_index: u32,
    /// That queue's index in its family.
    pub queue_index: u32,
    /// `vkGetInstanceProcAddr`, the loader's: cast it to `PFN_vkGetInstanceProcAddr`.
    pub get_instance_proc_addr: usize,
    /// Every device extension the device was created with (wgpu's own too).
    pub device_extensions: Vec<&'static CStr>,
    /// The physical device's ids (what [`VulkanRequest::physical_device`] matches), when the
    /// driver reports them.
    pub device_ids: Option<VulkanDeviceIds>,
}

/// The `VkImage` of a wgpu texture and the `VkFormat` it was created with.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VulkanImage {
    /// `VkImage`.
    pub image: u64,
    /// `VkFormat`, as its raw value.
    pub format: i32,
}

/// The requested names that are offered, each once, in the order they were requested.
/// What is not offered is logged and left out.
pub fn grant(what: &str, offered: &[&CStr], requested: &[&'static CStr]) -> Vec<&'static CStr> {
    let mut granted: Vec<&'static CStr> = Vec::new();
    for &name in requested {
        if granted.contains(&name) {
            continue;
        }
        if offered.contains(&name) {
            granted.push(name);
        } else {
            log::info!("Vulkan {what} extension {name:?} asked by a mod is not offered here");
        }
    }
    granted
}

/// The adapter the last Vulkan device was opened from (its formats) and what was granted.
struct State {
    adapter: Option<wgpu::Adapter>,
    granted: VulkanGranted,
}

static STATE: Mutex<State> = Mutex::new(State {
    adapter: None,
    granted: VulkanGranted {
        instance_extensions: Vec::new(),
        device_extensions: Vec::new(),
        timeline_semaphore: false,
        present_id: false,
    },
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What the last Vulkan instance and device were granted, once a device is open.
pub fn granted() -> Option<VulkanGranted> {
    let state = state();
    state.adapter.as_ref()?;
    Some(state.granted.clone())
}

/// A Vulkan instance created with the offered ones of `names`, when `descriptor` asks
/// for Vulkan alone and `names` is not empty. `None` leaves creating it to wgpu.
pub fn create_instance(
    descriptor: &wgpu::InstanceDescriptor,
    names: &[&'static CStr],
) -> Option<wgpu::Instance> {
    if names.is_empty() || descriptor.backends != wgpu::Backends::VULKAN {
        return None;
    }
    imp::create_instance(descriptor, names)
}

/// Opens a Vulkan device with what `request` asks for, where offered (and HDR's
/// metadata extension, as [`crate::open_vulkan_device`] does). `None` leaves opening
/// to wgpu, as with no mod. Remembers the adapter for [`image`] and [`format`].
pub fn open_device(
    adapter: &wgpu::Adapter,
    desc: &wgpu::DeviceDescriptor<'_>,
    request: &VulkanRequest,
) -> Option<Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError>> {
    if adapter.get_info().backend != wgpu::Backend::Vulkan {
        return None;
    }
    {
        let mut state = state();
        state.adapter = Some(adapter.clone());
        state.granted.device_extensions.clear();
        state.granted.timeline_semaphore = false;
        state.granted.present_id = false;
    }
    if !request.asks_device() {
        return no_mod_device(adapter, desc);
    }
    imp::open_device(adapter, desc, request)
}

#[cfg(not(target_os = "macos"))]
fn no_mod_device(
    adapter: &wgpu::Adapter,
    desc: &wgpu::DeviceDescriptor<'_>,
) -> Option<Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError>> {
    crate::open_vulkan_device(adapter, desc)
}

#[cfg(target_os = "macos")]
fn no_mod_device(
    _adapter: &wgpu::Adapter,
    _desc: &wgpu::DeviceDescriptor<'_>,
) -> Option<Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError>> {
    None
}

/// The raw handles of `device`; `None` unless it is a Vulkan device.
pub fn handles(device: &wgpu::Device) -> Option<VulkanHandles> {
    imp::handles(device)
}

/// The `VkImage` and `VkFormat` of `texture`; `None` unless it is a Vulkan texture.
pub fn image(texture: &wgpu::Texture) -> Option<VulkanImage> {
    let image = imp::image(texture)?;
    Some(VulkanImage {
        image,
        format: format(texture.format())?,
    })
}

/// The `VkImageView` of `view`; `None` unless it is a Vulkan view.
pub fn view(view: &wgpu::TextureView) -> Option<u64> {
    imp::view(view)
}

/// The `VkFormat` wgpu uses for `format` on the open device's adapter (a view may have
/// another format than its texture).
pub fn format(format: wgpu::TextureFormat) -> Option<i32> {
    let adapter = state().adapter.clone()?;
    imp::format(&adapter, format)
}

/// The texture uses wgpu-hal tracks for a texture made with `usage` in `format`: what
/// [`texture_from_vk_image`] tells wgpu the image may be used for.
pub fn hal_uses(usage: wgpu::TextureUsages, format: wgpu::TextureFormat) -> wgpu::TextureUses {
    let mut uses = wgpu::TextureUses::empty();
    if usage.contains(wgpu::TextureUsages::COPY_SRC) {
        uses |= wgpu::TextureUses::COPY_SRC;
    }
    if usage.contains(wgpu::TextureUsages::COPY_DST) {
        uses |= wgpu::TextureUses::COPY_DST;
    }
    if usage.contains(wgpu::TextureUsages::TEXTURE_BINDING) {
        uses |= wgpu::TextureUses::RESOURCE;
    }
    if usage.contains(wgpu::TextureUsages::STORAGE_BINDING) {
        uses |= wgpu::TextureUses::STORAGE_READ_ONLY
            | wgpu::TextureUses::STORAGE_WRITE_ONLY
            | wgpu::TextureUses::STORAGE_READ_WRITE;
    }
    if usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT) {
        uses |= if format.is_depth_stencil_format() {
            wgpu::TextureUses::DEPTH_STENCIL_READ | wgpu::TextureUses::DEPTH_STENCIL_WRITE
        } else {
            wgpu::TextureUses::COLOR_TARGET
        };
    }
    uses
}

/// A `wgpu::Texture` on `device` over `image`, a `VkImage` made outside wgpu, such as an OpenXR
/// swapchain image (`xrEnumerateSwapchainImages`). `desc` says what the image is: its size,
/// format, mip and sample counts, dimension and the uses the mod will put it to. `None` unless
/// `device` is a Vulkan device.
///
/// wgpu never destroys the image: the texture only borrows it. wgpu takes its contents as
/// undefined when it first uses it, so clear it or draw all of it (a render pass with
/// `LoadOp::Clear`). wgpu leaves the image in the layout of its last use in a submitted command
/// buffer: an OpenXR colour swapchain image must be back in `COLOR_ATTACHMENT_OPTIMAL` when it
/// is released, so make a render pass that draws into it its last use.
///
/// # Safety
/// - `image` is a live `VkImage` of `device`'s `VkDevice` (`handles(device).device`), created
///   with `desc`'s format (as [`format`] maps it), size, mip count, sample count and dimension,
///   and with at least the Vulkan usage flags `desc.usage` needs.
/// - The image outlives the texture, every view of it and every command buffer that uses it
///   until the GPU is done with them (for a swapchain image: until its `xrReleaseSwapchainImage`
///   after the submit, and drop the texture before the swapchain is destroyed).
pub unsafe fn texture_from_vk_image(
    device: &wgpu::Device,
    image: u64,
    desc: &wgpu::TextureDescriptor<'_>,
) -> Option<wgpu::Texture> {
    // Safety: forwarded from the caller.
    unsafe { imp::texture_from_vk_image(device, image, desc) }
}

/// Runs `f` with the raw `VkQueue` of `queue` (as its handle value) while no wgpu submit,
/// present or wait-idle runs on it, so a runtime that submits on the game's queue (OpenXR's
/// `xrBeginFrame`, `xrEndFrame`, `xrReleaseSwapchainImage` with a Vulkan graphics binding) gets
/// the external synchronization Vulkan requires. `None`, without calling `f`, unless `queue` is
/// a Vulkan queue.
///
/// Keep `f` short, and never submit or present through wgpu inside it: the lock is not
/// reentrant, so that deadlocks. One gap stays: wgpu's `vkDeviceWaitIdle` when it recreates the
/// window swapchain does not take this lock.
pub fn with_raw_queue<R>(queue: &wgpu::Queue, f: impl FnOnce(u64) -> R) -> Option<R> {
    imp::with_raw_queue(queue, f)
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::{VulkanDeviceIds, VulkanHandles, VulkanRequest, grant, state};
    use ash::vk::{self, Handle};
    use std::ffi::CStr;
    use wgpu::hal::api::Vulkan;

    pub fn create_instance(
        descriptor: &wgpu::InstanceDescriptor,
        names: &[&'static CStr],
    ) -> Option<wgpu::Instance> {
        let mut granted = Vec::new();
        // Safety: wgpu's own instance creation; the callback only adds extensions the
        // loader reports.
        let raw = unsafe {
            wgpu::hal::vulkan::Instance::init_with_callback(
                &wgpu::hal::InstanceDescriptor {
                    name: "Horizon",
                    flags: descriptor.flags,
                    memory_budget_thresholds: descriptor.memory_budget_thresholds,
                    backend_options: descriptor.backend_options.clone(),
                    telemetry: None,
                    // Vulkan reads no display handle while it creates the instance.
                    display: None,
                },
                Some(Box::new(|args| {
                    let offered = args
                        .entry
                        .enumerate_instance_extension_properties(None)
                        .unwrap_or_default();
                    let offered: Vec<&CStr> = offered
                        .iter()
                        .filter_map(|e| e.extension_name_as_c_str().ok())
                        .collect();
                    granted = grant("instance", &offered, names);
                    for &name in &granted {
                        if !args.extensions.contains(&name) {
                            args.extensions.push(name);
                        }
                    }
                })),
            )
        };
        match raw {
            Ok(raw) => {
                state().granted.instance_extensions = granted;
                // Safety: a Vulkan instance wgpu-hal made.
                Some(unsafe { wgpu::Instance::from_hal::<Vulkan>(raw) })
            }
            Err(e) => {
                log::warn!("Vulkan instance with the mods' extensions: {e}; opening without");
                None
            }
        }
    }

    pub fn open_device(
        adapter: &wgpu::Adapter,
        desc: &wgpu::DeviceDescriptor<'_>,
        request: &VulkanRequest,
    ) -> Option<Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError>> {
        let mut granted_extensions = Vec::new();
        let mut present_id_on = false;
        let timeline;
        let open = {
            // Safety: only read and opened from here; dropped before wgpu takes it back.
            let hal = unsafe { adapter.as_hal::<Vulkan>() }?;
            let shared = hal.shared_instance();
            let instance = shared.raw_instance();
            let physical = hal.raw_physical_device();
            // Safety: the adapter's own physical device on its own instance.
            let offered = unsafe { instance.enumerate_device_extension_properties(physical) }
                .unwrap_or_default();
            let offered: Vec<&CStr> = offered
                .iter()
                .filter_map(|e| e.extension_name_as_c_str().ok())
                .collect();
            let wanted = grant("device", &offered, &request.device_extensions);
            let hdr = offered.contains(&ash::ext::hdr_metadata::NAME);
            let api = shared
                .instance_api_version()
                .min(hal.physical_device_capabilities().properties().api_version);
            // Vulkan 1.2 makes timeline semaphores core and required, and the extension
            // requires its feature; wgpu turns them on in both cases.
            timeline = request.timeline_semaphore
                && (api >= vk::API_VERSION_1_2
                    || offered.contains(&ash::khr::timeline_semaphore::NAME));
            let mut present_id = vk::PhysicalDevicePresentIdFeaturesKHR::default();
            if request.present_id
                && offered.contains(&ash::khr::present_id::NAME)
                && shared.instance_api_version() >= vk::API_VERSION_1_1
            {
                let mut features =
                    vk::PhysicalDeviceFeatures2::default().push_next(&mut present_id);
                // Safety: a 1.1 instance's query of its own physical device.
                unsafe { instance.get_physical_device_features2(physical, &mut features) };
            } else if request.present_id {
                log::info!("Vulkan present id asked by a mod is not offered here");
            }
            let wants_present_id = present_id.present_id == vk::TRUE;
            // Safety: what wgpu's own request_device does, with offered extensions and
            // a supported feature added.
            unsafe {
                hal.open_with_callback(
                    desc.required_features,
                    &desc.required_limits,
                    &desc.memory_hints,
                    Some(Box::new(
                        |args: wgpu::hal::vulkan::CreateDeviceCallbackArgs| {
                            let mut add = |name: &'static CStr| {
                                if !args.extensions.contains(&name) {
                                    args.extensions.push(name);
                                }
                            };
                            if hdr {
                                add(ash::ext::hdr_metadata::NAME);
                            }
                            for &name in &wanted {
                                add(name);
                            }
                            if wants_present_id {
                                add(ash::khr::present_id::NAME);
                                *args.create_info = (*args.create_info).push_next(&mut present_id);
                                present_id_on = true;
                            }
                            granted_extensions = wanted.clone();
                        },
                    )),
                )
            }
            .ok()?
        };
        // Safety: opened from this adapter with exactly `desc`'s features.
        let opened = unsafe { adapter.create_device_from_hal(open, desc) };
        if opened.is_ok() {
            let mut state = state();
            state.granted.device_extensions = granted_extensions;
            state.granted.timeline_semaphore = timeline;
            state.granted.present_id = present_id_on;
        }
        Some(opened)
    }

    pub fn handles(device: &wgpu::Device) -> Option<VulkanHandles> {
        // Safety: read-only access to handles wgpu keeps alive while the guard lives.
        let hal = unsafe { device.as_hal::<Vulkan>() }?;
        let shared = hal.shared_instance();
        Some(VulkanHandles {
            instance: shared.raw_instance().handle().as_raw(),
            physical_device: hal.raw_physical_device().as_raw(),
            device: hal.raw_device().handle().as_raw(),
            queue: hal.raw_queue().as_raw(),
            queue_family_index: hal.queue_family_index(),
            queue_index: hal.queue_index(),
            get_instance_proc_addr: shared.entry().static_fn().get_instance_proc_addr as usize,
            device_extensions: hal.enabled_device_extensions().to_vec(),
            device_ids: ids(
                shared.raw_instance(),
                shared.instance_api_version(),
                hal.raw_physical_device(),
            ),
        })
    }

    pub fn device_ids(adapter: &wgpu::Adapter) -> Option<VulkanDeviceIds> {
        // Safety: read-only queries of the adapter's own physical device.
        let hal = unsafe { adapter.as_hal::<Vulkan>() }?;
        let shared = hal.shared_instance();
        ids(
            shared.raw_instance(),
            shared.instance_api_version(),
            hal.raw_physical_device(),
        )
    }

    /// `VkPhysicalDeviceIDProperties` of `physical`: core in Vulkan 1.1, which the instance and
    /// the device must both have.
    fn ids(
        instance: &ash::Instance,
        instance_api: u32,
        physical: vk::PhysicalDevice,
    ) -> Option<VulkanDeviceIds> {
        // Safety: a query of a physical device of this instance.
        let api = unsafe { instance.get_physical_device_properties(physical) }.api_version;
        if instance_api.min(api) < vk::API_VERSION_1_1 {
            return None;
        }
        let mut id = vk::PhysicalDeviceIDProperties::default();
        let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
        // Safety: a 1.1 instance's query of its own physical device.
        unsafe { instance.get_physical_device_properties2(physical, &mut properties) };
        Some(VulkanDeviceIds {
            uuid: id.device_uuid,
            luid: (id.device_luid_valid == vk::TRUE).then_some(id.device_luid),
        })
    }

    pub fn image(texture: &wgpu::Texture) -> Option<u64> {
        // Safety: read-only; the image lives as long as the texture.
        let hal = unsafe { texture.as_hal::<Vulkan>() }?;
        // Safety: the handle is only read.
        Some(unsafe { hal.raw_handle() }.as_raw())
    }

    pub fn view(view: &wgpu::TextureView) -> Option<u64> {
        // Safety: read-only; the view lives as long as the wgpu view.
        let hal = unsafe { view.as_hal::<Vulkan>() }?;
        // Safety: the handle is only read.
        Some(unsafe { hal.raw_handle() }.as_raw())
    }

    pub unsafe fn texture_from_vk_image(
        device: &wgpu::Device,
        image: u64,
        desc: &wgpu::TextureDescriptor<'_>,
    ) -> Option<wgpu::Texture> {
        let hal_desc = wgpu::hal::TextureDescriptor {
            label: desc.label,
            size: desc.size,
            mip_level_count: desc.mip_level_count,
            sample_count: desc.sample_count,
            dimension: desc.dimension,
            format: desc.format,
            usage: super::hal_uses(desc.usage, desc.format),
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: desc.view_formats.to_vec(),
        };
        let raw = {
            // Safety: the hal device is only used to wrap the image; the guard drops here.
            let hal = unsafe { device.as_hal::<Vulkan>() }?;
            // Safety: the caller vouches for the image. A drop callback marks it as not
            // wgpu's: wgpu-hal destroys only images without one.
            unsafe {
                hal.texture_from_raw(
                    vk::Image::from_raw(image),
                    &hal_desc,
                    Some(Box::new(|| {})),
                    wgpu::hal::vulkan::TextureMemory::External,
                )
            }
        };
        // Safety: a texture of this device's hal, described by `desc`; its contents start
        // undefined.
        Some(unsafe {
            device.create_texture_from_hal::<Vulkan>(raw, desc, wgpu::TextureUses::UNINITIALIZED)
        })
    }

    pub fn with_raw_queue<R>(queue: &wgpu::Queue, f: impl FnOnce(u64) -> R) -> Option<R> {
        // Safety: the raw queue is only handed to `f` under the hal queue's lock.
        let hal = unsafe { queue.as_hal::<Vulkan>() }?;
        Some(hal.with_raw(|raw| f(raw.as_raw())))
    }

    pub fn format(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> Option<i32> {
        // Safety: a read-only lookup on the adapter.
        let hal = unsafe { adapter.as_hal::<Vulkan>() }?;
        Some(hal.texture_format_as_raw(format).as_raw())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{VulkanDeviceIds, VulkanHandles, VulkanRequest};
    use std::ffi::CStr;

    pub fn create_instance(
        _descriptor: &wgpu::InstanceDescriptor,
        _names: &[&'static CStr],
    ) -> Option<wgpu::Instance> {
        None
    }
    pub fn open_device(
        _adapter: &wgpu::Adapter,
        _desc: &wgpu::DeviceDescriptor<'_>,
        _request: &VulkanRequest,
    ) -> Option<Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError>> {
        None
    }
    pub fn handles(_device: &wgpu::Device) -> Option<VulkanHandles> {
        None
    }
    pub fn device_ids(_adapter: &wgpu::Adapter) -> Option<VulkanDeviceIds> {
        None
    }
    pub unsafe fn texture_from_vk_image(
        _device: &wgpu::Device,
        _image: u64,
        _desc: &wgpu::TextureDescriptor<'_>,
    ) -> Option<wgpu::Texture> {
        None
    }
    pub fn with_raw_queue<R>(_queue: &wgpu::Queue, _f: impl FnOnce(u64) -> R) -> Option<R> {
        None
    }
    pub fn image(_texture: &wgpu::Texture) -> Option<u64> {
        None
    }
    pub fn view(_view: &wgpu::TextureView) -> Option<u64> {
        None
    }
    pub fn format(_adapter: &wgpu::Adapter, _format: wgpu::TextureFormat) -> Option<i32> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_offered_extensions_are_granted_once_in_request_order() {
        let offered = [c"VK_KHR_b", c"VK_KHR_a", c"VK_KHR_c"];
        let requested = [c"VK_KHR_c", c"VK_HORIZON_missing", c"VK_KHR_a", c"VK_KHR_c"];
        assert_eq!(
            grant("device", &offered, &requested),
            vec![c"VK_KHR_c", c"VK_KHR_a"]
        );
    }

    #[test]
    fn nothing_offered_grants_nothing_without_failing() {
        assert!(grant("instance", &[], &[c"VK_HORIZON_missing"]).is_empty());
    }

    #[test]
    fn the_physical_device_asked_for_is_picked_by_uuid_or_valid_luid() {
        let a = VulkanDeviceIds::new([1; 16], Some([1; 8]));
        let b = VulkanDeviceIds::new([2; 16], None);
        let c = VulkanDeviceIds::new([3; 16], Some([3; 8]));
        let adapters = [None, Some(a), Some(b), Some(c)];
        assert_eq!(
            pick_device(&adapters, &VulkanDeviceId::Uuid([2; 16])),
            Some(2)
        );
        assert_eq!(
            pick_device(&adapters, &VulkanDeviceId::Luid([3; 8])),
            Some(3)
        );
        // b reports no valid LUID: an all-zero LUID never matches it.
        assert_eq!(pick_device(&adapters, &VulkanDeviceId::Luid([0; 8])), None);
        assert_eq!(pick_device(&adapters, &VulkanDeviceId::Uuid([9; 16])), None);
        assert_eq!(pick_device(&[], &VulkanDeviceId::Uuid([1; 16])), None);
    }

    #[test]
    fn asking_for_a_physical_device_is_asking() {
        let request = VulkanRequest {
            physical_device: Some(VulkanDeviceId::Uuid([7; 16])),
            ..Default::default()
        };
        assert!(!request.is_empty());
        // It is no device extension or feature: the device opens as it would.
        assert!(!request.asks_device());
    }

    #[test]
    fn no_physical_device_asked_picks_none_and_none_found_falls_back() {
        assert_eq!(wanted_adapter(&[], &VulkanRequest::default()), None);
        let request = VulkanRequest {
            physical_device: Some(VulkanDeviceId::Uuid([0xAB; 16])),
            ..Default::default()
        };
        assert_eq!(wanted_adapter(&[], &request), None);
    }

    /// A Vulkan adapter of this machine, if it has one (tests skip without a GPU).
    #[cfg(not(target_os = "macos"))]
    fn vulkan_adapter() -> Option<wgpu::Adapter> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(descriptor);
        pollster::block_on(instance.request_adapter(&Default::default())).ok()
    }

    /// What the adapter's physical device offers, read straight from the driver.
    #[cfg(not(target_os = "macos"))]
    fn offered(adapter: &wgpu::Adapter) -> Vec<std::ffi::CString> {
        // Safety: read-only queries of the adapter's own physical device.
        let hal = unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }.unwrap();
        let properties = unsafe {
            hal.shared_instance()
                .raw_instance()
                .enumerate_device_extension_properties(hal.raw_physical_device())
        }
        .unwrap();
        properties
            .iter()
            .filter_map(|e| e.extension_name_as_c_str().ok().map(CStr::to_owned))
            .collect()
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_device_gets_the_offered_extensions_asked_for_and_skips_the_rest() {
        let Some(adapter) = vulkan_adapter() else {
            eprintln!("no Vulkan adapter: skipped");
            return;
        };
        let offered = offered(&adapter);
        let asked = [
            c"VK_HORIZON_not_a_real_extension",
            c"VK_KHR_push_descriptor",
            c"VK_EXT_memory_budget",
            c"VK_KHR_shader_clock",
        ];
        let request = VulkanRequest {
            device_extensions: asked.to_vec(),
            ..Default::default()
        };
        let (device, _queue) = open_device(&adapter, &Default::default(), &request)
            .expect("a Vulkan adapter opens here")
            .expect("an extension that is not offered is no error");
        let handles = handles(&device).unwrap();
        for name in asked {
            assert_eq!(
                handles.device_extensions.contains(&name),
                offered.iter().any(|o| o.as_c_str() == name),
                "{name:?} enabled exactly when the driver offers it"
            );
        }
        assert!(handles.instance != 0 && handles.physical_device != 0 && handles.device != 0);
        assert!(handles.queue != 0 && handles.get_instance_proc_addr != 0);
        assert_eq!(handles.device_ids, device_ids(&adapter));

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let image = image(&texture).unwrap();
        assert_ne!(image.image, 0);
        assert_eq!(image.format, ash::vk::Format::R8G8B8A8_UNORM.as_raw());
        assert_ne!(view(&texture.create_view(&Default::default())).unwrap(), 0);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn an_adapter_is_found_by_its_own_ids() {
        let Some(adapter) = vulkan_adapter() else {
            eprintln!("no Vulkan adapter: skipped");
            return;
        };
        let Some(ids) = device_ids(&adapter) else {
            eprintln!("no Vulkan 1.1 adapter: skipped");
            return;
        };
        let adapters = [adapter];
        let by_uuid = VulkanRequest {
            physical_device: Some(VulkanDeviceId::Uuid(ids.uuid)),
            ..Default::default()
        };
        assert_eq!(wanted_adapter(&adapters, &by_uuid), Some(0));
        if let Some(luid) = ids.luid {
            let by_luid = VulkanRequest {
                physical_device: Some(VulkanDeviceId::Luid(luid)),
                ..Default::default()
            };
            assert_eq!(wanted_adapter(&adapters, &by_luid), Some(0));
        }
        let mut other = ids.uuid;
        other[0] ^= 0xFF;
        let elsewhere = VulkanRequest {
            physical_device: Some(VulkanDeviceId::Uuid(other)),
            ..Default::default()
        };
        assert_eq!(wanted_adapter(&adapters, &elsewhere), None);
    }

    #[test]
    fn texture_usages_map_to_the_uses_wgpu_hal_tracks() {
        use wgpu::{TextureFormat as F, TextureUsages as U, TextureUses as H};
        assert_eq!(
            hal_uses(U::RENDER_ATTACHMENT | U::COPY_DST, F::Rgba8UnormSrgb),
            H::COLOR_TARGET | H::COPY_DST
        );
        assert_eq!(
            hal_uses(U::RENDER_ATTACHMENT, F::Depth32Float),
            H::DEPTH_STENCIL_READ | H::DEPTH_STENCIL_WRITE
        );
        assert_eq!(
            hal_uses(U::TEXTURE_BINDING | U::COPY_SRC, F::Bgra8Unorm),
            H::RESOURCE | H::COPY_SRC
        );
        assert!(hal_uses(U::STORAGE_BINDING, F::Rgba16Float).contains(H::STORAGE_READ_WRITE));
        assert!(hal_uses(U::empty(), F::Rgba8Unorm).is_empty());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn an_image_made_elsewhere_is_drawn_into_as_a_texture() {
        let Some(adapter) = vulkan_adapter() else {
            eprintln!("no Vulkan adapter: skipped");
            return;
        };
        let (device, queue) = open_device(&adapter, &Default::default(), &VulkanRequest::default())
            .unwrap()
            .unwrap();
        let desc = wgpu::TextureDescriptor {
            label: Some("external"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        // The "runtime's" image: a texture of wgpu's own, so the test needs no raw Vulkan.
        let owner = device.create_texture(&desc);
        let raw = image(&owner).unwrap().image;
        // Safety: `owner` keeps the image alive past `wrapped` and the submit below.
        let wrapped = unsafe { texture_from_vk_image(&device, raw, &desc) }.unwrap();
        assert_eq!(image(&wrapped).unwrap().image, raw);
        let mut encoder = device.create_command_encoder(&Default::default());
        let view = wrapped.create_view(&Default::default());
        drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        }));
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wrapped.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(4),
                },
            },
            desc.size,
        );
        queue.submit([encoder.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(
            &buffer.slice(..).get_mapped_range().unwrap()[..4],
            &[255, 0, 0, 255]
        );
        drop(view);
        drop(wrapped);
        drop(owner);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn the_raw_queue_is_lent_between_wgpu_submits() {
        let Some(adapter) = vulkan_adapter() else {
            eprintln!("no Vulkan adapter: skipped");
            return;
        };
        let (device, queue) = open_device(&adapter, &Default::default(), &VulkanRequest::default())
            .unwrap()
            .unwrap();
        let raw = with_raw_queue(&queue, |raw| raw).unwrap();
        assert_eq!(raw, handles(&device).unwrap().queue);
        // wgpu submits again once the closure returned.
        queue.submit([device.create_command_encoder(&Default::default()).finish()]);
        // A submit on another thread waits for the lent queue instead of racing it.
        let order = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            with_raw_queue(&queue, |_| {
                let submitter = scope.spawn(|| {
                    queue.submit([device.create_command_encoder(&Default::default()).finish()]);
                    order.lock().unwrap().push("submit");
                });
                std::thread::sleep(std::time::Duration::from_millis(50));
                order.lock().unwrap().push("raw");
                submitter
            })
            .unwrap();
        });
        assert_eq!(*order.lock().unwrap(), ["raw", "submit"]);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn with_nothing_asked_the_device_opens_as_without_mods() {
        let Some(adapter) = vulkan_adapter() else {
            eprintln!("no Vulkan adapter: skipped");
            return;
        };
        let enabled = |device: &wgpu::Device| {
            let mut names = handles(device).unwrap().device_extensions;
            names.sort();
            names
        };
        let (plain, _q) = crate::open_vulkan_device(&adapter, &Default::default())
            .unwrap()
            .unwrap();
        let (hooked, _q) = open_device(&adapter, &Default::default(), &VulkanRequest::default())
            .unwrap()
            .unwrap();
        assert_eq!(enabled(&hooked), enabled(&plain));
        assert_eq!(hooked.features(), plain.features());
        assert!(VulkanRequest::default().is_empty());
    }
}
