use horizon_client_render::render_slots::{self, Slots};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn render_provider_is_optional_and_installed_once() {
    assert!(!render_slots::world_drawn_hooked());
    assert!(!render_slots::needs().entity_spans);
    assert!(!render_slots::needs().world_depth);
    assert!(
        render_slots::vulkan_request().is_empty(),
        "no provider asks Vulkan for nothing"
    );
    render_slots::install(Slots {
        world_views: |_| {},
        world_views_hooked: || false,
        world_view_drawn: |_| {},
        world_drawn: |_| {},
        hand_drawn: |_| {},
        render_needs: |needs| {
            needs.entity_spans = true;
            needs.world_depth = true;
        },
        world_drawn_hooked: || true,
        vulkan_device: |request| {
            request.device_extensions.push(c"VK_KHR_a");
            request.device_extensions.push(c"VK_KHR_b");
            request.present_id = true;
        },
        device_ready: |ready| {
            READY.fetch_add(1, Ordering::SeqCst);
            assert_eq!(ready.device.limits(), ready.adapter.limits());
        },
    });
    assert!(render_slots::world_drawn_hooked());
    assert!(render_slots::needs().entity_spans);
    assert!(render_slots::needs().world_depth);
    let request = render_slots::vulkan_request();
    assert_eq!(request.device_extensions, [c"VK_KHR_a", c"VK_KHR_b"]);
    assert!(request.present_id && !request.is_empty());
    render_slots::install(Slots {
        world_views: |_| {},
        world_views_hooked: || false,
        world_view_drawn: |_| {},
        world_drawn: |_| panic!("second provider replaced the first"),
        hand_drawn: |_| panic!("second provider replaced the first"),
        render_needs: |_| panic!("second provider replaced the first"),
        world_drawn_hooked: || false,
        vulkan_device: |_| panic!("second provider replaced the first"),
        device_ready: |_| panic!("second provider replaced the first"),
    });
    assert!(render_slots::world_drawn_hooked());
    assert!(render_slots::needs().world_depth);
    assert!(render_slots::vulkan_request().present_id);
    device_ready_reaches_the_first_provider();
}

static READY: AtomicUsize = AtomicUsize::new(0);

/// The installed provider hears of a device (any backend this machine has; skipped without one).
fn device_ready_reaches_the_first_provider() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: device_ready skipped");
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    render_slots::device_ready(&mut render_slots::DeviceReady {
        adapter: &adapter,
        device: &device,
        queue: &queue,
    });
    assert_eq!(READY.load(Ordering::SeqCst), 1);
}
