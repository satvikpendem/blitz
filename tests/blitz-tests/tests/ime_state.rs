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
    assert_eq!(*values.lock().unwrap(), ["A🦀Z", "Aé🦀Z"]);
}

#[test]
fn native_composition_updates_publish_live_values_without_appending_twice() {
    let (mut doc, id, values) = editor();
    state(&mut doc, &values, "A", (1, 1), None);
    state(&mut doc, &values, "Aé", (3, 3), Some((1, 3)));
    state(&mut doc, &values, "A東京", (7, 7), Some((1, 7)));
    assert_eq!(*values.lock().unwrap(), ["A", "Aé", "A東京"]);
    state(&mut doc, &values, "A東京", (7, 7), None);
    state(&mut doc, &values, "A東京", (7, 7), None);
    assert_eq!(*values.lock().unwrap(), ["A", "Aé", "A東京", "A東京"]);
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

#[test]
fn controlled_value_echo_preserves_active_composition_and_selection() {
    let (mut doc, id, values) = editor();
    state(&mut doc, &values, "Aé🦀Z", (8, 8), None);
    state(&mut doc, &values, "Aé東京Z", (9, 3), Some((3, 9)));
    let value_attr = blitz_dom::QualName::new(
        None,
        blitz_dom::Namespace::from(""),
        blitz_dom::LocalName::from("value"),
    );
    // A controlled UI writes its latest Input value back into the DOM. The
    // unchanged raw text must not cancel the IME's still-active composing span.
    doc.mutate()
        .set_attribute(id, value_attr.clone(), "Aé東京Z");
    doc.resolve(0.0);
    let editor = &doc
        .get_node(id)
        .unwrap()
        .element_data()
        .unwrap()
        .text_input_data()
        .unwrap()
        .editor;
    assert_eq!(editor.raw_text(), "Aé東京Z");
    assert_eq!(*editor.raw_compose(), Some(3..9));
    assert_eq!(editor.raw_selection().anchor().index(), 9);
    assert_eq!(editor.raw_selection().focus().index(), 3);

    // An actual external text change still replaces the buffer and ends IME
    // composition, without fabricating a user Input event.
    doc.mutate().set_attribute(id, value_attr, "External");
    doc.resolve(0.0);
    let editor = &doc
        .get_node(id)
        .unwrap()
        .element_data()
        .unwrap()
        .text_input_data()
        .unwrap()
        .editor;
    assert_eq!(editor.raw_text(), "External");
    assert!(editor.raw_compose().is_none());
    assert_eq!(*values.lock().unwrap(), ["Aé🦀Z", "Aé東京Z"]);
}

#[test]
fn composing_only_snapshot_dispatches_the_visible_input_value() {
    let (mut doc, _, values) = editor();
    state(&mut doc, &values, "東京 🦀", (11, 11), Some((0, 11)));
    assert_eq!(*values.lock().unwrap(), ["東京 🦀"]);
    // A selection-only update does not fabricate another text input.
    state(&mut doc, &values, "東京 🦀", (0, 6), Some((0, 11)));
    assert_eq!(*values.lock().unwrap(), ["東京 🦀"]);
}

#[test]
fn controlled_clear_replaces_an_uncommitted_composition() {
    let (mut doc, id, values) = editor();
    state(&mut doc, &values, "東京 🦀", (11, 11), Some((0, 11)));
    let value_attr = blitz_dom::QualName::new(
        None,
        blitz_dom::Namespace::from(""),
        blitz_dom::LocalName::from("value"),
    );
    doc.mutate().set_attribute(id, value_attr, "");
    doc.resolve(0.0);
    let editor = &doc
        .get_node(id)
        .unwrap()
        .element_data()
        .unwrap()
        .text_input_data()
        .unwrap()
        .editor;
    assert_eq!(editor.raw_text(), "");
    assert!(editor.raw_compose().is_none());
}

#[test]
fn preedit_events_publish_visible_changes_and_cancellation() {
    let (mut doc, _, values) = editor();
    for event in [
        BlitzImeEvent::Preedit("東京".into(), Some((6, 6))),
        BlitzImeEvent::Preedit("東京".into(), Some((0, 6))),
        BlitzImeEvent::Disabled,
    ] {
        EventDriver::new(&mut doc, Handler(values.clone())).handle_ui_event(UiEvent::Ime(event));
    }
    assert_eq!(*values.lock().unwrap(), ["東京", ""]);
}
