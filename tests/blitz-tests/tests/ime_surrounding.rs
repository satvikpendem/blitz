use blitz_dom::{Document, DocumentConfig, EventDriver, EventHandler};
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::{
    events::{BlitzImeEvent, BlitzKeyEvent, DomEvent, DomEventData, EventState, KeyState, UiEvent},
    node_id::NodeId,
    shell::{ColorScheme, Viewport},
};
use keyboard_types::{Code, Key, Location, Modifiers, NamedKey};
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
fn event(doc: &mut HtmlDocument, values: &Arc<Mutex<Vec<String>>>, event: UiEvent) {
    EventDriver::new(doc, Handler(values.clone())).handle_ui_event(event);
}
fn key(doc: &mut HtmlDocument, values: &Arc<Mutex<Vec<String>>>, shift: bool) {
    event(
        doc,
        values,
        UiEvent::KeyDown(BlitzKeyEvent {
            key: Key::Named(NamedKey::ArrowLeft),
            code: Code::ArrowLeft,
            modifiers: if shift {
                Modifiers::SHIFT
            } else {
                Modifiers::empty()
            },
            location: Location::Standard,
            is_auto_repeating: false,
            is_composing: false,
            state: KeyState::Pressed,
            text: None,
        }),
    );
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
    event(&mut doc, &values, UiEvent::Focus(id));
    event(
        &mut doc,
        &values,
        UiEvent::Ime(BlitzImeEvent::Commit("Aé🦀Z".into())),
    );
    (doc, id, values)
}
fn text(doc: &HtmlDocument, id: NodeId) -> &str {
    doc.get_node(id)
        .unwrap()
        .element_data()
        .unwrap()
        .text_input_data()
        .unwrap()
        .editor
        .raw_text()
}
fn delete(doc: &mut HtmlDocument, values: &Arc<Mutex<Vec<String>>>, before: usize, after: usize) {
    event(
        doc,
        values,
        UiEvent::Ime(BlitzImeEvent::DeleteSurrounding {
            before_bytes: before,
            after_bytes: after,
        }),
    );
}

#[test]
fn surrounding_deletion_uses_utf8_bytes_and_emits_live_input() {
    let (mut doc, id, values) = editor();
    key(&mut doc, &values, false); // caret before Z
    delete(&mut doc, &values, 4, 1); // remove crab and Z
    assert_eq!(text(&doc, id), "Aé");
    assert_eq!(*values.lock().unwrap(), ["Aé🦀Z", "Aé"]);
}

#[test]
fn surrounding_deletion_preserves_reversed_selection() {
    let (mut doc, id, values) = editor();
    key(&mut doc, &values, false);
    key(&mut doc, &values, true);
    key(&mut doc, &values, true); // select é and crab backwards
    delete(&mut doc, &values, usize::MAX, usize::MAX);
    assert_eq!(text(&doc, id), "é🦀");
    event(
        &mut doc,
        &values,
        UiEvent::Ime(BlitzImeEvent::Commit("X".into())),
    );
    assert_eq!(text(&doc, id), "X");
}

#[test]
fn invalid_unicode_boundaries_and_zero_counts_do_not_emit_input() {
    let (mut doc, id, values) = editor();
    key(&mut doc, &values, false);
    let count = values.lock().unwrap().len();
    delete(&mut doc, &values, 1, 0); // would split the crab's UTF-8 encoding
    delete(&mut doc, &values, 0, 0);
    assert_eq!(text(&doc, id), "Aé🦀Z");
    assert_eq!(values.lock().unwrap().len(), count);
    delete(&mut doc, &values, 1, 1); // invalid before range, valid after range
    assert_eq!(text(&doc, id), "Aé🦀");
    delete(&mut doc, &values, usize::MAX, 0);
    assert_eq!(text(&doc, id), "");
}
