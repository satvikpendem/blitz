use blitz_dom::{Document, DocumentConfig, EventDriver, EventHandler};
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::{
    events::{BlitzImeEvent, DomEvent, DomEventData, EventState, UiEvent},
    node_id::NodeId,
    shell::{ColorScheme, Viewport},
};
use std::sync::{Arc, Mutex};

struct Handler(Arc<Mutex<Vec<String>>>);
impl EventHandler for Handler {
    fn handle_event(
        &mut self,
        _: &[NodeId],
        event: &mut DomEvent,
        _: &mut dyn Document,
        _: &mut EventState,
    ) {
        if let DomEventData::Input(input) = &event.data {
            self.0.lock().unwrap().push(input.value.clone());
        }
    }
}
fn editor() -> (HtmlDocument, NodeId, Arc<Mutex<Vec<String>>>) {
    let mut doc = HtmlDocument::from_html(
        "<input id=title>",
        DocumentConfig {
            viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
            ..Default::default()
        },
    );
    doc.resolve(0.0);
    let id = doc.get_element_by_id("title").unwrap();
    let values = Arc::new(Mutex::new(Vec::new()));
    EventDriver::new(&mut doc, Handler(values.clone())).handle_ui_event(UiEvent::Focus(id));
    (doc, id, values)
}
fn state(
    doc: &mut HtmlDocument,
    values: &Arc<Mutex<Vec<String>>>,
    text: &str,
    selection: (usize, usize),
    composition: Option<(usize, usize)>,
) {
    EventDriver::new(doc, Handler(values.clone())).handle_ui_event(UiEvent::Ime(
        BlitzImeEvent::SetState {
            text: text.into(),
            selection,
            composition,
        },
    ));
}

#[test]
fn native_snapshot_preserves_unicode_selection_and_composition() {
    let (mut doc, id, values) = editor();
    state(&mut doc, &values, "A🦀Z", (1, 1), None);
    state(&mut doc, &values, "Aé🦀Z", (7, 3), Some((1, 3)));
    let editor = &doc
        .get_node(id)
        .unwrap()
        .element_data()
        .unwrap()
        .text_input_data()
        .unwrap()
        .editor;
    assert_eq!(editor.raw_text(), "Aé🦀Z");
    assert_eq!(editor.text(), "A🦀Z");
    assert_eq!(*editor.raw_compose(), Some(1..3));
    assert_eq!(editor.raw_selection().anchor().index(), 7);
    assert_eq!(editor.raw_selection().focus().index(), 3);
    assert_eq!(*values.lock().unwrap(), ["A🦀Z"]);
}

#[test]
fn native_composition_updates_do_not_commit_or_append_twice() {
    let (mut doc, id, values) = editor();
    state(&mut doc, &values, "A", (1, 1), None);
    state(&mut doc, &values, "Aé", (3, 3), Some((1, 3)));
    state(&mut doc, &values, "A東京", (7, 7), Some((1, 7)));
    assert_eq!(*values.lock().unwrap(), ["A"]);
    state(&mut doc, &values, "A東京", (7, 7), None);
    state(&mut doc, &values, "A東京", (7, 7), None);
    assert_eq!(*values.lock().unwrap(), ["A", "A東京"]);
    assert_eq!(
        doc.get_node(id)
            .unwrap()
            .element_data()
            .unwrap()
            .text_input_data()
            .unwrap()
            .editor
            .raw_text(),
        "A東京"
    );
}

#[test]
fn malformed_native_offsets_leave_editor_and_events_unchanged() {
    let (mut doc, id, values) = editor();
    for (selection, composition) in [
        ((1, 1), None),
        ((0, 9), None),
        ((0, 0), Some((2, 0))),
        ((0, 0), Some((0, 1))),
        ((0, 0), Some((0, 0))),
    ] {
        state(&mut doc, &values, "é🦀", selection, composition);
    }
    assert_eq!(
        doc.get_node(id)
            .unwrap()
            .element_data()
            .unwrap()
            .text_input_data()
            .unwrap()
            .editor
            .raw_text(),
        ""
    );
    assert!(values.lock().unwrap().is_empty());
}
