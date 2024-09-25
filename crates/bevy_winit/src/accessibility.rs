//! Helpers for mapping window entities to accessibility types

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use bevy_a11y::{
    accesskit::ActionRequest, AccessibilityNode, AccessibilityRequested, AccessibilitySystem, Focus,
};
use bevy_a11y::{ActionRequest as ActionRequestWrapper, ManageAccessibilityUpdates};
use bevy_app::{App, Plugin, PostUpdate};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::entity::EntityHashMap;
use bevy_ecs::{
    prelude::{Entity, EventReader, EventWriter},
    query::With,
    schedule::IntoSystemConfigs,
    system::{NonSendMut, Query, Res, ResMut, Resource},
};
use bevy_hierarchy::{Children, Parent};
use bevy_window::{PrimaryWindow, Window, WindowClosed};

/// Maps window entities to their `AccessKit` [`Adapter`]s.
#[derive(Default, Deref, DerefMut)]
pub struct AccessKitAdapters(pub EntityHashMap<()>);

/// Maps window entities to their respective [`WinitActionRequests`]s.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct WinitActionRequestHandlers(pub EntityHashMap<Arc<Mutex<WinitActionRequestHandler>>>);

/// Forwards `AccessKit` [`ActionRequest`]s from winit to an event channel.
#[derive(Clone, Default, Deref, DerefMut)]
pub struct WinitActionRequestHandler(pub VecDeque<ActionRequest>);

impl WinitActionRequestHandler {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self(VecDeque::new())))
    }
}

/// Prepares accessibility for a winit window.
pub(crate) fn prepare_accessibility_for_window(
    _winit_window: &winit::window::Window,
    entity: Entity,
    _name: String,
    _accessibility_requested: AccessibilityRequested,
    adapters: &mut AccessKitAdapters,
    handlers: &mut WinitActionRequestHandlers,
) {
    let action_request_handler = WinitActionRequestHandler::new();

    adapters.insert(entity, ());
    handlers.insert(entity, action_request_handler);
}

fn window_closed(
    mut adapters: NonSendMut<AccessKitAdapters>,
    mut handlers: ResMut<WinitActionRequestHandlers>,
    mut events: EventReader<WindowClosed>,
) {
    for WindowClosed { window, .. } in events.read() {
        adapters.remove(window);
        handlers.remove(window);
    }
}

fn poll_receivers(
    handlers: Res<WinitActionRequestHandlers>,
    mut actions: EventWriter<ActionRequestWrapper>,
) {
    for (_id, handler) in handlers.iter() {
        let mut handler = handler.lock().unwrap();
        while let Some(event) = handler.pop_front() {
            actions.send(ActionRequestWrapper(event));
        }
    }
}

fn should_update_accessibility_nodes(
    accessibility_requested: Res<AccessibilityRequested>,
    manage_accessibility_updates: Res<ManageAccessibilityUpdates>,
) -> bool {
    accessibility_requested.get() && manage_accessibility_updates.get()
}

fn update_accessibility_nodes(
    mut _adapters: NonSendMut<AccessKitAdapters>,
    _focus: Res<Focus>,
    _primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    _nodes: Query<(
        Entity,
        &AccessibilityNode,
        Option<&Children>,
        Option<&Parent>,
    )>,
    _node_entities: Query<Entity, With<AccessibilityNode>>,
) {
}

/// Implements winit-specific `AccessKit` functionality.
pub struct AccessKitPlugin;

impl Plugin for AccessKitPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<AccessKitAdapters>()
            .init_resource::<WinitActionRequestHandlers>()
            .add_event::<ActionRequestWrapper>()
            .add_systems(
                PostUpdate,
                (
                    poll_receivers,
                    update_accessibility_nodes.run_if(should_update_accessibility_nodes),
                    window_closed
                        .before(poll_receivers)
                        .before(update_accessibility_nodes),
                )
                    .in_set(AccessibilitySystem::Update),
            );
    }
}
