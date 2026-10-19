use crate::{BlitzShellEvent, event::BlitzShellProxy};
use accesskit::{Affine, NodeId, Rect};
use accesskit_xplat::{Adapter, EventHandler, WindowEvent as AccessKitEvent};
use blitz_dom::BaseDocument;
use std::sync::Arc;
#[cfg(not(target_os = "android"))]
use winit::raw_window_handle::HasWindowHandle;
use winit::{
    event::WindowEvent,
    window::{Window, WindowId},
};

/// State of the accessibility node tree and platform adapter.
pub struct AccessibilityState {
    // /// Adapter to connect to the [`EventLoop`](`winit::event_loop::EventLoop`).
    adapter: Adapter,
}

struct Handler {
    window_id: WindowId,
    proxy: BlitzShellProxy,
}
impl EventHandler for Handler {
    fn handle_accesskit_event(&self, event: AccessKitEvent) {
        self.proxy.send_event(BlitzShellEvent::Accessibility {
            window_id: self.window_id,
            data: Arc::new(event),
        });
    }
}

impl AccessibilityState {
    pub fn new(window: &dyn Window, proxy: BlitzShellProxy) -> Self {
        let window_id = window.id();
        Self {
            adapter: Adapter::with_combined_handler(
                #[cfg(target_os = "android")]
                &crate::current_android_app(),
                #[cfg(not(target_os = "android"))]
                window.window_handle().unwrap().as_raw(),
                Arc::new(Handler { window_id, proxy }),
            ),
        }
    }
    pub fn update_tree(&mut self, doc: &BaseDocument, offset: (u32, u32)) {
        self.adapter.update_if_active(|| {
            let mut tree = doc.build_accessibility_tree();
            // The DOM uses CSS pixels; the platform tree uses physical surface
            // coordinates, including the same safe-area origin as painting.
            if let Some((_, root)) = tree
                .nodes
                .iter_mut()
                .find(|(id, _)| *id == NodeId(u64::MAX))
            {
                let scale = doc.viewport().scale_f64();
                root.set_transform(Affine::new([
                    scale,
                    0.0,
                    0.0,
                    scale,
                    offset.0 as f64,
                    offset.1 as f64,
                ]));
            }
            tree
        });
    }

    /// Allows reacting to window events.
    ///
    /// This must be called whenever a new window event is received
    /// and before it is handled by the application.
    pub fn process_window_event(&mut self, window: &dyn Window, event: &WindowEvent) {
        match event {
            WindowEvent::Focused(is_focused) => {
                self.adapter.set_focus(*is_focused);
            }
            WindowEvent::Moved(_) | WindowEvent::SurfaceResized(_) => {
                let outer_position: (_, _) = window
                    .outer_position()
                    .unwrap_or_default()
                    .cast::<f64>()
                    .into();
                let outer_size: (_, _) = window.outer_size().cast::<f64>().into();
                let inner_position: (_, _) = window.surface_position().cast::<f64>().into();
                let inner_size: (_, _) = window.surface_size().cast::<f64>().into();

                self.adapter.set_window_bounds(
                    Rect::from_origin_size(outer_position, outer_size),
                    Rect::from_origin_size(inner_position, inner_size),
                )
            }
            _ => (),
        }
    }
}
