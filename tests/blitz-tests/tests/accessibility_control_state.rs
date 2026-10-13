//! Explicit control names and live checkedness must reach the platform tree.
use accesskit::{Node, NodeId, Toggled};
use blitz_dom::{DocumentConfig, QualName, local_name, ns};
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::shell::{ColorScheme, Viewport};
use std::sync::Arc;

fn doc(html: &str) -> HtmlDocument {
    let mut doc = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
            ..Default::default()
        },
    );
    doc.resolve(0.0);
    doc
}
fn node(doc: &HtmlDocument, id: &str) -> Node {
    let id = NodeId(doc.get_element_by_id(id).unwrap().as_u64());
    doc.build_accessibility_tree()
        .nodes
        .into_iter()
        .find(|(candidate, _)| *candidate == id)
        .unwrap()
        .1
}
#[test]
fn explicit_names_survive_unicode_and_dynamic_updates() {
    let mut doc = doc(r#"<button id=edit aria-label="Edit Café 東京">✎</button>"#);
    assert_eq!(node(&doc, "edit").label(), Some("Edit Café 東京"));
    let id = doc.get_element_by_id("edit").unwrap();
    let name = QualName::new(None, ns!(), local_name!("aria-label"));
    doc.mutate().set_attribute(id, name.clone(), "Delete task");
    assert_eq!(node(&doc, "edit").label(), Some("Delete task"));
    doc.mutate().clear_attribute(id, name);
    assert_eq!(node(&doc, "edit").label(), None);
}
#[test]
fn checkbox_tree_uses_live_state_instead_of_initial_attribute() {
    let mut doc = doc("<input id=task type=checkbox checked><input id=radio type=radio>");
    assert_eq!(node(&doc, "task").toggled(), Some(Toggled::True));
    assert_eq!(node(&doc, "radio").toggled(), Some(Toggled::False));
    let id = doc.get_element_by_id("task").unwrap();
    doc.get_node_mut(id)
        .unwrap()
        .element_data_mut()
        .unwrap()
        .set_checkbox_input_checked(false);
    // Pointer-driven checkedness leaves the initial content attribute present.
    assert!(
        doc.get_node(id)
            .unwrap()
            .element_data()
            .unwrap()
            .has_attr(local_name!("checked"))
    );
    assert_eq!(node(&doc, "task").toggled(), Some(Toggled::False));
}
#[test]
fn empty_explicit_name_does_not_replace_content_labelling() {
    let doc = doc(r#"<button id=task aria-label="  ">Save</button>"#);
    assert_eq!(node(&doc, "task").label(), None);
    assert!(!node(&doc, "task").labelled_by().is_empty());
}
