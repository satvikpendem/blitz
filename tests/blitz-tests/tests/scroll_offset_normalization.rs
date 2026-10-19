//! Reflow must normalize every scroll offset, independently of the focused node.
use blitz_dom::{DocumentConfig, ScrollBehavior};
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::shell::{ColorScheme, Viewport};
use std::sync::Arc;
use std::time::Duration;

fn doc(html: &str, height: u32) -> HtmlDocument {
    let mut d = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(Viewport::new(400, height, 1.0, ColorScheme::Light)),
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
            ..Default::default()
        },
    );
    d.resolve(0.0);
    d
}
fn panel_doc(overflow: &str, expand: bool) -> HtmlDocument {
    let height = if expand { "height:auto;" } else { "" };
    doc(
        &format!(
            "<style>body{{margin:0}}header{{height:80px}}input{{height:44px;width:160px}}#panel{{width:240px;height:120px;overflow:auto}}#content{{width:800px;height:1000px}}#leaf{{width:20px;height:30px}}@media(max-height:600px){{#panel{{overflow:{overflow};{height}}}}}</style><header id=header></header><input id=editor value='Café 東京 🦀'><div id=panel><div id=content><div id=leaf></div></div></div>"
        ),
        700,
    )
}
fn offset(d: &HtmlDocument, name: &str) -> (f64, f64) {
    let node = d.get_node(d.get_element_by_id(name).unwrap()).unwrap();
    (node.scroll_offset().x, node.scroll_offset().y)
}
fn scroll(d: &mut HtmlDocument, name: &str, x: f64, y: f64, behavior: ScrollBehavior) {
    let id = d.get_element_by_id(name).unwrap();
    d.scroll_to(id, x, y, behavior);
}
fn set_style(d: &mut HtmlDocument, name: &str, property: &str, value: &str) {
    let id = d.get_element_by_id(name).unwrap();
    d.mutate().set_style_property(id, property, value);
}
fn scroll_then_shrink(d: &mut HtmlDocument) {
    scroll(d, "panel", 37.0, 471.0, ScrollBehavior::Instant);
    assert_eq!(offset(d, "panel"), (37.0, 471.0));
    d.viewport_mut().window_size = (400, 400);
    d.resolve(1.0);
}
fn assert_panel_contents_start_at_its_border(d: &HtmlDocument) {
    assert_eq!(offset(d, "panel"), (0.0, 0.0));
    let panel = d
        .get_client_bounding_rect(d.get_element_by_id("panel").unwrap())
        .unwrap();
    let leaf = d
        .get_client_bounding_rect(d.get_element_by_id("leaf").unwrap())
        .unwrap();
    assert_eq!((leaf.x, leaf.y), (panel.x, panel.y));
}

#[test]
fn visible_overflow_clears_unfocused_sibling_panel_offset() {
    let mut d = panel_doc("visible", true);
    scroll_then_shrink(&mut d);
    assert_panel_contents_start_at_its_border(&d);
}
#[test]
fn visible_overflow_clears_panel_outside_focused_editor_chain() {
    let mut d = panel_doc("visible", true);
    let editor = d.get_element_by_id("editor").unwrap();
    d.set_focus_to(editor);
    d.resolve(0.5);
    scroll_then_shrink(&mut d);
    assert_panel_contents_start_at_its_border(&d);
}
#[test]
fn clip_overflow_discards_previous_scroll_offset() {
    let mut d = panel_doc("clip", false);
    scroll_then_shrink(&mut d);
    assert_panel_contents_start_at_its_border(&d);
}
#[test]
fn hidden_overflow_preserves_valid_programmatic_scroll_offset() {
    let mut d = panel_doc("hidden", false);
    scroll_then_shrink(&mut d);
    assert_eq!(offset(&d, "panel"), (37.0, 471.0));
    scroll(&mut d, "panel", 52.0, 510.0, ScrollBehavior::Instant);
    d.resolve(2.0);
    assert_eq!(offset(&d, "panel"), (52.0, 510.0));
}
#[test]
fn content_shrink_clamps_both_axes_to_fresh_layout_range() {
    let mut d = panel_doc("auto", false);
    scroll(&mut d, "panel", 300.0, 471.0, ScrollBehavior::Instant);
    set_style(&mut d, "content", "width", "300px");
    set_style(&mut d, "content", "height", "180px");
    d.resolve(1.0);
    assert_eq!(offset(&d, "panel"), (60.0, 60.0));
}
#[test]
fn enlarged_scrollport_clamps_without_any_focused_node() {
    let mut d = panel_doc("auto", false);
    set_style(&mut d, "panel", "height", "calc(100vh - 100px)");
    d.viewport_mut().window_size = (400, 300);
    d.resolve(0.5);
    scroll(&mut d, "panel", 37.0, 471.0, ScrollBehavior::Instant);
    d.viewport_mut().window_size = (400, 1200);
    d.resolve(1.0);
    assert_eq!(offset(&d, "panel"), (37.0, 0.0));
}
#[test]
fn valid_offsets_survive_reflow_and_unrelated_resolves() {
    let mut d = panel_doc("auto", false);
    scroll(&mut d, "panel", 37.0, 63.0, ScrollBehavior::Instant);
    d.viewport_mut().window_size = (400, 400);
    set_style(&mut d, "header", "height", "95px");
    d.resolve(1.0);
    d.resolve(2.0);
    assert_eq!(offset(&d, "panel"), (37.0, 63.0));
}
#[test]
fn smooth_animation_cannot_reintroduce_offset_beyond_shrunken_range() {
    let mut d = panel_doc("auto", false);
    scroll(&mut d, "panel", 0.0, 400.0, ScrollBehavior::Instant);
    scroll(&mut d, "panel", 0.0, 800.0, ScrollBehavior::Smooth);
    set_style(&mut d, "content", "height", "180px");
    for time in [1.0, 2.0] {
        d.resolve(time);
        assert_eq!(offset(&d, "panel"), (0.0, 60.0));
    }
    std::thread::sleep(Duration::from_millis(330));
    d.resolve(3.0);
    assert_eq!(offset(&d, "panel"), (0.0, 60.0));
}
#[test]
fn normalizing_one_panel_does_not_cancel_another_panels_smooth_scroll() {
    let mut d = doc(
        "<style>body{margin:0}.port{width:240px;height:100px;overflow:auto}.content{height:1000px}@media(max-height:600px){#panel{overflow:visible;height:auto}}</style><div id=panel class=port><div class=content></div></div><div id=other class=port><div class=content></div></div>",
        700,
    );
    scroll(&mut d, "panel", 0.0, 471.0, ScrollBehavior::Instant);
    scroll(&mut d, "other", 0.0, 800.0, ScrollBehavior::Smooth);
    d.viewport_mut().window_size = (400, 400);
    d.resolve(1.0);
    assert_eq!(offset(&d, "panel"), (0.0, 0.0));
    std::thread::sleep(Duration::from_millis(330));
    d.resolve(2.0);
    assert_eq!(offset(&d, "other"), (0.0, 800.0));
}
#[test]
fn content_shrink_clamps_viewport_after_fresh_layout() {
    let mut d = doc(
        "<style>body{margin:0}#content{height:1600px}</style><div id=content></div>",
        700,
    );
    let root = d.root_element().id;
    d.scroll_to(root, 0.0, 800.0, ScrollBehavior::Instant);
    set_style(&mut d, "content", "height", "300px");
    d.resolve(1.0);
    assert_eq!(d.viewport_scroll().y, 0.0);
}
