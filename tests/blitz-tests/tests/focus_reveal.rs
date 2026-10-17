//! Root viewport focus visibility after focus and keyboard/rotation resizes.
use blitz_dom::{DocumentConfig, ScrollBehavior};
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::shell::{ColorScheme, Viewport};
use std::sync::Arc;

fn doc(height: u32, scale: f32) -> HtmlDocument {
    custom_doc(
        height,
        scale,
        "<style>body{margin:0}#spacer{height:300px}input{height:48px;width:240px;box-sizing:border-box}#tail{height:600px}</style><div id=spacer></div><input id=edit value='Café 東京 🦀'><div id=tail></div>",
    )
}
fn custom_doc(height: u32, scale: f32, html: &str) -> HtmlDocument {
    let mut doc = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(Viewport::new(
                (400.0 * scale) as u32,
                (height as f32 * scale) as u32,
                scale,
                ColorScheme::Light,
            )),
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
            ..Default::default()
        },
    );
    doc.resolve(0.0);
    doc
}
fn focus(doc: &mut HtmlDocument) {
    let id = doc.get_element_by_id("edit").unwrap();
    doc.set_focus_to(id);
    doc.resolve(1.0);
}
fn assert_visible(doc: &HtmlDocument) {
    let id = doc.get_element_by_id("edit").unwrap();
    let r = doc.get_client_bounding_rect(id).unwrap();
    let (_, h) = doc.viewport().logical_size();
    assert!(
        r.y >= -1.0 && r.y + r.height <= h as f64 + 1.0,
        "focused input {r:?} outside viewport height {h}, scroll={:?}",
        doc.viewport_scroll()
    );
    assert_eq!(
        doc.get_node(id)
            .unwrap()
            .element_data()
            .unwrap()
            .text_input_data()
            .unwrap()
            .editor
            .raw_text(),
        "Café 東京 🦀"
    );
}
#[test]
fn keyboard_resize_reveals_focused_input_at_fractional_scales() {
    for scale in [1.0, 1.25, 2.625, 3.0] {
        let mut d = doc(700, scale);
        focus(&mut d);
        d.viewport_mut().window_size = ((900.0 * scale) as u32, (150.0 * scale) as u32);
        d.resolve(2.0);
        assert_visible(&d);
    }
}
#[test]
fn new_focus_reveals_input_below_fold_after_layout() {
    let mut d = doc(150, 1.0);
    focus(&mut d);
    assert_visible(&d);
}
#[test]
fn visible_focus_does_not_move_page() {
    let mut d = doc(700, 1.0);
    focus(&mut d);
    assert_eq!(d.viewport_scroll().y, 0.0);
    assert_visible(&d);
}
#[test]
fn later_resolves_do_not_fight_user_scroll() {
    let mut d = doc(150, 1.0);
    focus(&mut d);
    let root = d.root_element().id;
    d.scroll_to(root, 0.0, 500.0, ScrollBehavior::Instant);
    let before = d.viewport_scroll();
    d.resolve(2.0);
    d.resolve(3.0);
    assert_eq!(d.viewport_scroll(), before);
}
#[test]
fn color_scheme_change_does_not_reveal_again() {
    let mut d = doc(150, 1.0);
    focus(&mut d);
    let root = d.root_element().id;
    d.scroll_to(root, 0.0, 500.0, ScrollBehavior::Instant);
    let before = d.viewport_scroll();
    d.viewport_mut().color_scheme = ColorScheme::Dark;
    d.resolve(2.0);
    assert_eq!(d.viewport_scroll(), before);
}
#[test]
fn cleared_focus_cancels_queued_resize_reveal() {
    let mut d = doc(700, 1.0);
    focus(&mut d);
    d.viewport_mut().window_size = (400, 150);
    d.clear_focus();
    d.resolve(2.0);
    assert_eq!(d.viewport_scroll().y, 0.0);
}
#[test]
fn removed_focus_target_is_safe_before_resize_resolve() {
    let mut d = doc(700, 1.0);
    focus(&mut d);
    d.viewport_mut().window_size = (400, 150);
    let id = d.get_element_by_id("edit").unwrap();
    d.mutate().remove_and_drop_node(id);
    d.resolve(2.0);
    assert_eq!(d.viewport_scroll().y, 0.0);
}
#[test]
fn coalesced_resizes_reveal_using_final_geometry() {
    let mut d = doc(700, 1.0);
    focus(&mut d);
    d.viewport_mut().window_size = (900, 90);
    d.viewport_mut().window_size = (900, 150);
    d.resolve(2.0);
    assert_visible(&d);
}
#[test]
fn enlarged_viewport_clamps_old_scroll_range() {
    let mut d = doc(150, 1.0);
    focus(&mut d);
    let root = d.root_element().id;
    d.scroll_to(root, 0.0, 500.0, ScrollBehavior::Instant);
    d.viewport_mut().window_size = (400, 1000);
    d.resolve(2.0);
    assert_eq!(d.viewport_scroll().y, 0.0);
    assert_visible(&d);
}

#[test]
fn hidden_editor_on_responsive_resize_preserves_manual_scroll() {
    for rule in [
        "#edit{display:none}",
        "#wrap{display:none}",
        "#edit{visibility:hidden}",
        "#wrap{visibility:hidden}",
    ] {
        let html = format!(
            "<style>body{{margin:0}}#spacer{{height:300px}}input{{height:48px}}#tail{{height:2000px}}@media(min-width:800px){{{rule}}}</style><div id=spacer></div><div id=wrap><input id=edit value='Café 東京 🦀'></div><div id=tail></div>"
        );
        let mut d = custom_doc(700, 1.0, &html);
        focus(&mut d);
        let root = d.root_element().id;
        d.scroll_to(root, 0.0, 500.0, ScrollBehavior::Instant);
        let before = d.viewport_scroll();
        d.viewport_mut().window_size = (900, 700);
        d.resolve(2.0);
        assert_eq!(d.viewport_scroll(), before, "hidden by {rule}");
    }
}

#[test]
fn visible_child_of_visibility_hidden_ancestor_is_revealed() {
    let mut d = custom_doc(
        150,
        1.0,
        "<style>body{margin:0}#spacer{height:300px}#wrap{visibility:hidden}input{visibility:visible;height:48px}#tail{height:600px}</style><div id=spacer></div><div id=wrap><input id=edit value='Café 東京 🦀'></div><div id=tail></div>",
    );
    focus(&mut d);
    assert_visible(&d);
}

#[test]
fn viewport_fixed_editor_preserves_background_scroll_on_focus_and_resize() {
    for rule in ["#edit", "#wrap"] {
        let html = format!(
            "<style>body{{margin:0;height:2000px}}input{{height:48px}}{rule}{{position:fixed;top:20px}}</style><div id=wrap><input id=edit value='Café 東京 🦀'></div>"
        );
        let mut d = custom_doc(700, 1.0, &html);
        let root = d.root_element().id;
        d.scroll_to(root, 0.0, 500.0, ScrollBehavior::Instant);
        let before = d.viewport_scroll();
        focus(&mut d);
        assert_eq!(d.viewport_scroll(), before, "focus with fixed {rule}");
        d.viewport_mut().window_size = (900, 150);
        d.resolve(2.0);
        assert_eq!(d.viewport_scroll(), before, "resize with fixed {rule}");
    }
}

#[test]
fn fixed_editor_contained_by_transform_still_reveals_in_document_flow() {
    let mut d = custom_doc(
        150,
        1.0,
        "<style>body{margin:0}#spacer{height:300px}#wrap{transform:translate(0px,0px);height:60px}input{position:fixed;top:0;height:48px}#tail{height:600px}</style><div id=spacer></div><div id=wrap><input id=edit value='Café 東京 🦀'></div><div id=tail></div>",
    );
    focus(&mut d);
    assert_visible(&d);
}
