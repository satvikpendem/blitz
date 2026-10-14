use accesskit::{Action, ActionRequest, NodeId, TreeId};
use blitz_dom::{Document, DocumentConfig, EventDriver, EventHandler};
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::events::{DomEvent, EventState};
use blitz_traits::shell::{ColorScheme, Viewport};
use std::sync::{Arc, Mutex};

fn doc(html: &str, scale: f32) -> HtmlDocument {
    let mut doc = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(Viewport::new(800, 600, scale, ColorScheme::Light)),
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
            ..Default::default()
        },
    );
    doc.resolve(0.0);
    doc
}
fn request(doc: &HtmlDocument, id: &str, action: Action) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: NodeId(doc.get_element_by_id(id).unwrap().as_u64()),
        data: None,
    }
}

#[test]
fn semantic_activation_targets_checkbox_under_overlay_and_emits_input() {
    let mut doc = doc(
        r#"<input id=task type=checkbox><div style="position:fixed;inset:0;z-index:9">Overlay</div>"#,
        1.0,
    );
    let request = request(&doc, "task", Action::Click);
    let event = doc.accessibility_action_event(&request).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    struct Handler(Arc<Mutex<Vec<String>>>);
    impl EventHandler for Handler {
        fn handle_event(
            &mut self,
            _: &[blitz_traits::node_id::NodeId],
            event: &mut DomEvent,
            _: &mut dyn Document,
            _: &mut EventState,
        ) {
            self.0.lock().unwrap().push(event.name().to_string());
        }
    }
    EventDriver::new(&mut doc, Handler(events.clone())).handle_ui_event(event);
    let id = doc.get_element_by_id("task").unwrap();
    assert_eq!(
        doc.get_node(id)
            .unwrap()
            .element_data()
            .unwrap()
            .checkbox_input_checked(),
        Some(true)
    );
    let events = events.lock().unwrap();
    assert!(events.iter().any(|name| name == "click"));
    assert!(events.iter().any(|name| name == "input"));
}

#[test]
fn disabled_hidden_and_stale_targets_do_not_accept_actions() {
    let mut doc = doc(
        r#"<button id=disabled disabled>Disabled</button><button id=aria aria-disabled=true>Disabled</button><div style="display:none"><button id=hidden>Hidden</button></div><div aria-hidden=true><button id=ariahidden>Hidden</button></div><button id=old>Old</button>"#,
        1.0,
    );
    for id in ["disabled", "aria", "hidden", "ariahidden"] {
        assert!(
            doc.accessibility_action_event(&request(&doc, id, Action::Click))
                .is_none()
        );
    }
    let old = request(&doc, "old", Action::Click);
    doc.mutate()
        .remove_and_drop_node(blitz_traits::node_id::NodeId::from_u64(old.target_node.0));
    assert!(doc.accessibility_action_event(&old).is_none());
    let mut foreign = request(&doc, "disabled", Action::Click);
    foreign.target_tree.0 = "00000000-0000-0000-0000-000000000123".parse().unwrap();
    assert!(doc.accessibility_action_event(&foreign).is_none());
}

#[test]
fn semantic_focus_updates_document_focus_and_rejects_nonfocusable_nodes() {
    let mut doc = doc(r#"<input id=title><div id=plain>Text</div>"#, 1.0);
    let event = doc
        .accessibility_action_event(&request(&doc, "title", Action::Focus))
        .unwrap();
    doc.handle_ui_event(event);
    assert_eq!(doc.get_focussed_node_id(), doc.get_element_by_id("title"));
    assert!(
        doc.accessibility_action_event(&request(&doc, "plain", Action::Focus))
            .is_none()
    );
}

#[test]
fn consumer_bounds_follow_device_scale_and_viewport_scroll() {
    let mut doc = doc(
        r#"<style>body{margin:0}button{position:absolute;left:40px;top:100px;width:80px;height:30px;padding:0;border:0}</style><button id=target>Load</button>"#,
        2.0,
    );
    let id = NodeId(doc.get_element_by_id("target").unwrap().as_u64());
    for scroll in [0.0, 50.0] {
        doc.set_viewport_scroll(blitz_dom::Point { x: 0.0, y: scroll });
        let tree = accesskit_consumer::Tree::new(doc.build_accessibility_tree(), true);
        let state = tree.state();
        let node = state.node_by_tree_local_id(id, TreeId::ROOT).unwrap();
        let bounds = node.bounding_box().unwrap();
        assert_eq!(
            bounds,
            accesskit::Rect::new(80.0, 200.0 - 2.0 * scroll, 240.0, 260.0 - 2.0 * scroll)
        );
    }
}
