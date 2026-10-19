//! Automatic editor reveal must include the outline actually painted outside its border box.
use blitz_dom::{DocumentConfig, ScrollBehavior, ScrollLogicalPosition};
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::shell::{ColorScheme, Viewport};
use std::sync::Arc;

fn doc(html: &str, height: u32, scale: f32) -> HtmlDocument {
    let mut d = HtmlDocument::from_html(
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
    d.resolve(0.0);
    d
}
fn focus(d: &mut HtmlDocument) {
    let id = d.get_element_by_id("edit").unwrap();
    d.set_focus_to(id);
    d.resolve(1.0);
}
#[derive(Debug)]
struct PaintedRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
fn painted_rect(d: &HtmlDocument, name: &str) -> PaintedRect {
    let node = d.get_node(d.get_element_by_id(name).unwrap()).unwrap();
    let p = node.absolute_position(node.scroll_offset().x as f32, node.scroll_offset().y as f32);
    let root = d.viewport_scroll();
    let size = node.final_layout().size;
    PaintedRect {
        x: p.x as f64 - root.x,
        y: p.y as f64 - root.y,
        width: size.width as f64,
        height: size.height as f64,
    }
}
fn painted_outline_width(d: &HtmlDocument) -> f64 {
    let node = d.get_node(d.get_element_by_id("edit").unwrap()).unwrap();
    let width = node
        .primary_styles()
        .unwrap()
        .get_outline()
        .outline_width
        .0
        .to_f64_px();
    assert!(
        width > 0.0,
        "the fixture must have a visible nonzero outline"
    );
    width
}
fn assert_root_outline(d: &HtmlDocument) {
    let width = painted_outline_width(d);
    let r = painted_rect(d, "edit");
    let (w, h) = d.viewport().logical_size();
    assert!(
        r.x - width >= -0.01
            && r.y - width >= -0.01
            && r.x + r.width + width <= w as f64 + 0.01
            && r.y + r.height + width <= h as f64 + 0.01,
        "outline {width} around {r:?} outside root {w}x{h}"
    );
}
fn assert_port_outline(d: &HtmlDocument, name: &str) {
    let width = painted_outline_width(d);
    let r = painted_rect(d, "edit");
    let p = painted_rect(d, name);
    let port = d.get_node(d.get_element_by_id(name).unwrap()).unwrap();
    let b = port.final_layout().border;
    let left = p.x + b.left as f64;
    let top = p.y + b.top as f64;
    assert!(
        r.x - width >= left - 0.01
            && r.y - width >= top - 0.01
            && r.x + r.width + width <= left + port.client_width() as f64 + 0.01
            && r.y + r.height + width <= top + port.client_height() as f64 + 0.01,
        "outline {width} around {r:?} outside {name} padding-box ({left},{top}) {}x{}",
        port.client_width(),
        port.client_height()
    );
}
const ROOT: &str = "<style>body{margin:0}#spacer{height:300px}input{display:block;margin-left:10px;width:220px;height:48px;box-sizing:border-box}#tail{height:600px}</style><div id=spacer></div><input id=edit value='Café 東京 🦀'><div id=tail></div>";

#[test]
fn root_reveal_keeps_default_focus_outline_inside_viewport() {
    for scale in [1.0, 1.25, 2.625, 3.0] {
        let mut d = doc(ROOT, 150, scale);
        focus(&mut d);
        assert_root_outline(&d);
    }
}
#[test]
fn keyboard_resize_keeps_focus_outline_inside_final_viewport() {
    for scale in [1.0, 1.25, 2.625, 3.0] {
        let mut d = doc(ROOT, 700, scale);
        focus(&mut d);
        d.viewport_mut().window_size = ((400.0 * scale) as u32, (150.0 * scale) as u32);
        d.resolve(2.0);
        assert_root_outline(&d);
    }
}
#[test]
fn visible_border_box_with_clipped_outline_still_scrolls() {
    let mut d = doc(
        "<style>body{margin:0}#spacer{height:116px}input{display:block;margin-left:10px;width:220px;height:32px;box-sizing:border-box}input:focus{outline:4px solid blue}#tail{height:500px}</style><div id=spacer></div><input id=edit><div id=tail></div>",
        150,
        1.0,
    );
    focus(&mut d);
    assert_root_outline(&d);
}
#[test]
fn multiline_editor_reveal_keeps_default_outline_inside_viewport() {
    for scale in [1.0, 1.25, 2.625, 3.0] {
        let mut d = doc(
            "<style>body{margin:0}#spacer{height:300px}textarea{display:block;margin-left:10px;width:220px;height:48px;box-sizing:border-box}#tail{height:600px}</style><div id=spacer></div><textarea id=edit>Café 東京 🦀\nSecond line</textarea><div id=tail></div>",
            150,
            scale,
        );
        focus(&mut d);
        assert_root_outline(&d);
    }
}
#[test]
fn nested_scrollports_reveal_complete_outline_inside_out() {
    for scale in [1.0, 1.25, 2.625, 3.0] {
        let mut d = doc(
            "<style>body{margin:0}#outer{margin:20px;width:330px;height:180px;overflow:auto;border:8px solid;padding:12px;box-sizing:border-box}#inner{margin-top:250px;width:270px;height:100px;overflow:auto;border:5px solid;padding:10px;box-sizing:border-box}#spacer{height:350px}input{display:block;margin-left:8px;width:160px;height:40px;box-sizing:border-box}#tail{height:600px}</style><div id=outer><div id=inner><div id=spacer></div><input id=edit><div id=tail></div></div><div id=tail></div></div><div id=tail></div>",
            150,
            scale,
        );
        focus(&mut d);
        assert_port_outline(&d, "inner");
        assert_port_outline(&d, "outer");
        assert_root_outline(&d);
    }
}
#[test]
fn horizontal_scrollport_reveals_right_outline() {
    let mut d = doc(
        "<style>body{margin:0}#port{margin:20px;width:220px;height:150px;overflow:auto;border:4px solid;box-sizing:border-box}#wide{position:relative;width:700px;height:100px}input{position:absolute;left:400px;top:35px;width:160px;height:40px;box-sizing:border-box}</style><div id=port><div id=wide><input id=edit></div></div>",
        700,
        2.625,
    );
    focus(&mut d);
    assert_port_outline(&d, "port");
}
#[test]
fn fully_visible_outline_does_not_move_page() {
    let mut d = doc(ROOT, 700, 1.0);
    focus(&mut d);
    assert_root_outline(&d);
    assert_eq!(d.viewport_scroll().y, 0.0);
}
#[test]
fn unpainted_outlines_do_not_add_a_reveal_margin() {
    for (style, color) in [
        ("none", "black"),
        ("auto", "black"),
        ("0 solid blue", "black"),
        ("2px solid transparent", "black"),
        ("2px solid currentColor", "transparent"),
    ] {
        let html = format!(
            "<style>body{{margin:0}}#spacer{{height:300px}}input{{display:block;margin-left:10px;width:220px;height:48px;box-sizing:border-box}}input:focus{{outline:{style};color:{color}}}#tail{{height:600px}}</style><div id=spacer></div><input id=edit><div id=tail></div>"
        );
        let mut d = doc(&html, 150, 1.0);
        focus(&mut d);
        let r = painted_rect(&d, "edit");
        assert!(
            (r.y + r.height - 150.0).abs() < 0.01,
            "{style}/{color}: {r:?}"
        );
    }
}
#[test]
fn public_scroll_into_view_still_uses_border_box_alignment() {
    let mut d = doc(ROOT, 150, 1.0);
    focus(&mut d);
    let id = d.get_element_by_id("edit").unwrap();
    d.scroll_into_view(
        id,
        ScrollBehavior::Instant,
        ScrollLogicalPosition::End,
        ScrollLogicalPosition::Nearest,
    );
    let r = d.get_client_bounding_rect(id).unwrap();
    assert!((r.y + r.height - 150.0).abs() < 0.01, "{r:?}");
}
