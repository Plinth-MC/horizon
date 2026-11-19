use horizon_client_render::render_slots::{self, Slots};

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
    });
    assert!(render_slots::world_drawn_hooked());
    assert!(render_slots::needs().world_depth);
    assert!(render_slots::vulkan_request().present_id);
}
