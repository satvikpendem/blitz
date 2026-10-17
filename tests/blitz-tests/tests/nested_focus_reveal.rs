//! Automatic focus reveal must reveal each enclosing scrollport before the page.
use blitz_dom::{DocumentConfig, ScrollBehavior};
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
    assert_eq!(
        d.get_node(id)
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
fn assert_in_port(d: &HtmlDocument, name: &str) {
    let target = d.get_node(d.get_element_by_id("edit").unwrap()).unwrap();
    let port = d.get_node(d.get_element_by_id(name).unwrap()).unwrap();
    let t = target.absolute_position(
        target.scroll_offset().x as f32,
        target.scroll_offset().y as f32,
    );
    let p = port.absolute_position(port.scroll_offset().x as f32, port.scroll_offset().y as f32);
    let b = port.final_layout().border;
    let size = target.final_layout().size;
    let x = p.x + b.left;
    let y = p.y + b.top;
    assert!(
        t.x >= x - 1.0
            && t.x + size.width <= x + port.client_width() + 1.0
            && t.y >= y - 1.0
            && t.y + size.height <= y + port.client_height() + 1.0,
        "editor {t:?}/{size:?} outside {name} padding port ({x},{y})/{}x{}, offset={:?}",
        port.client_width(),
        port.client_height(),
        port.scroll_offset()
    );
}
const ONE: &str = "<style>body{margin:0}#outer{margin-top:30px;height:140px;width:300px;overflow:auto;border:8px solid;padding:12px;box-sizing:border-box}#spacer{height:350px}input{height:44px;width:160px;box-sizing:border-box}#tail{height:600px}</style><div id=outer><div id=spacer></div><input id=edit value='Café 東京 🦀'><div id=tail></div></div><div style='height:1000px'></div>";

#[test]
fn one_scrollport_reveals_editor_without_moving_visible_page() {
    for scale in [1.0, 1.25, 2.625, 3.0] {
        let mut d = doc(ONE, 700, scale);
        focus(&mut d);
        assert_in_port(&d, "outer");
        assert_eq!(d.viewport_scroll().y, 0.0);
    }
}
#[test]
fn two_scrollports_reveal_inside_out_before_moving_page() {
    let html = "<style>body{margin:0}#outer{margin-top:40px;height:180px;width:340px;overflow:auto;border:8px solid;padding:12px;box-sizing:border-box}#inner{margin-top:250px;height:100px;width:270px;overflow:auto;border:5px solid;padding:10px;box-sizing:border-box}#spacer{height:400px}input{height:40px;width:160px;box-sizing:border-box}#tail{height:600px}</style><div id=outer><div id=inner><div id=spacer></div><input id=edit value='Café 東京 🦀'><div id=tail></div></div><div id=tail></div></div><div id=tail></div>";
    let mut d = doc(html, 700, 2.625);
    focus(&mut d);
    assert_in_port(&d, "inner");
    assert_in_port(&d, "outer");
    assert_eq!(d.viewport_scroll().y, 0.0);
    d.viewport_mut().window_size = (1050, 394);
    d.resolve(2.0);
    assert_in_port(&d, "inner");
    assert_in_port(&d, "outer");
    let r = d
        .get_client_bounding_rect(d.get_element_by_id("edit").unwrap())
        .unwrap();
    assert!(r.y >= -1.0 && r.y + r.height <= 151.0, "{r:?}");
}
#[test]
fn keyboard_shrink_reveals_again_in_resized_scrollport() {
    let mut d = doc(
        "<style>body{margin:0}#outer{height:100vh;width:300px;overflow:auto}#spacer{height:300px}input{height:44px;width:160px;box-sizing:border-box}#tail{height:600px}</style><div id=outer><div id=spacer></div><input id=edit value='Café 東京 🦀'><div id=tail></div></div>",
        700,
        1.0,
    );
    focus(&mut d);
    assert_in_port(&d, "outer");
    assert_eq!(
        d.get_node(d.get_element_by_id("outer").unwrap())
            .unwrap()
            .scroll_offset()
            .y,
        0.0
    );
    d.viewport_mut().window_size = (400, 150);
    d.resolve(2.0);
    assert_in_port(&d, "outer");
    assert_eq!(d.viewport_scroll().y, 0.0);
}
#[test]
fn hidden_overflow_scrolls_programmatically_but_clip_does_not() {
    for overflow in ["hidden", "clip"] {
        let html = format!(
            "<style>body{{margin:0}}#outer{{height:140px;width:300px;overflow:{overflow}}}#spacer{{height:300px}}input{{height:44px;width:160px;box-sizing:border-box}}#tail{{height:600px}}</style><div id=outer><div id=spacer></div><input id=edit value='Café 東京 🦀'><div id=tail></div></div><div id=tail></div>"
        );
        let mut d = doc(&html, 700, 1.0);
        focus(&mut d);
        if overflow == "hidden" {
            assert_in_port(&d, "outer")
        } else {
            assert_eq!(
                d.get_node(d.get_element_by_id("outer").unwrap())
                    .unwrap()
                    .scroll_offset()
                    .y,
                0.0
            );
        }
        assert_eq!(d.viewport_scroll().y, 0.0);
    }
}
#[test]
fn fixed_panel_scrolls_its_content_without_moving_background() {
    let mut d = doc(
        "<style>body{margin:0;height:2000px}#outer{position:fixed;top:20px;height:140px;width:300px;overflow:auto}#spacer{height:300px}input{height:44px;width:160px;box-sizing:border-box}#tail{height:600px}</style><div id=outer><div id=spacer></div><input id=edit value='Café 東京 🦀'><div id=tail></div></div>",
        700,
        1.0,
    );
    let root = d.root_element().id;
    d.scroll_to(root, 0.0, 500.0, ScrollBehavior::Instant);
    focus(&mut d);
    assert_in_port(&d, "outer");
    assert_eq!(d.viewport_scroll().y, 500.0);
}
#[test]
fn horizontal_panel_reveals_without_vertical_or_root_scroll() {
    let mut d = doc(
        "<style>body{margin:0}#outer{width:140px;height:100px;overflow:auto}#content{width:800px}input{display:block;margin-left:450px;height:44px;width:80px;box-sizing:border-box}</style><div id=outer><div id=content><input id=edit value='Café 東京 🦀'></div></div>",
        700,
        1.0,
    );
    focus(&mut d);
    assert_in_port(&d, "outer");
    assert_eq!(d.viewport_scroll().x, 0.0);
    assert_eq!(d.viewport_scroll().y, 0.0);
}
#[test]
fn manual_panel_scroll_survives_unrelated_resolves() {
    let mut d = doc(ONE, 700, 1.0);
    focus(&mut d);
    let port = d.get_element_by_id("outer").unwrap();
    d.scroll_to(port, 0.0, 0.0, ScrollBehavior::Instant);
    d.resolve(2.0);
    d.resolve(3.0);
    assert_eq!(d.get_node(port).unwrap().scroll_offset().y, 0.0);
}
#[test]
fn absolute_editor_escapes_non_containing_scrollport() {
    let mut d = doc(
        "<style>body{margin:0;position:relative;height:1500px}#outer{height:100px;overflow:auto}input{position:absolute;top:350px;left:20px;height:44px;width:160px;box-sizing:border-box}</style><div id=outer><input id=edit value='Café 東京 🦀'></div>",
        150,
        1.0,
    );
    focus(&mut d);
    assert_eq!(
        d.get_node(d.get_element_by_id("outer").unwrap())
            .unwrap()
            .scroll_offset()
            .y,
        0.0
    );
    let r = d
        .get_client_bounding_rect(d.get_element_by_id("edit").unwrap())
        .unwrap();
    assert!(r.y >= -1.0 && r.y + r.height <= 151.0, "{r:?}");
}
