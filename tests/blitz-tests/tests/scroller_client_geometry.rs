//! Scrolling an element moves its contents, not its own CSSOM/AccessKit border box.
use blitz_dom::{BoundingRect, DocumentConfig, ScrollBehavior};
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::shell::{ColorScheme, Viewport};
use std::sync::Arc;

fn doc(html: &str, scale: f32) -> HtmlDocument {
    let mut d = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(Viewport::new(
                (600.0 * scale) as u32,
                (500.0 * scale) as u32,
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
fn rect(d: &HtmlDocument, name: &str) -> BoundingRect {
    d.get_client_bounding_rect(d.get_element_by_id(name).unwrap())
        .unwrap()
}
fn scroll(d: &mut HtmlDocument, name: &str, x: f64, y: f64) -> (f64, f64) {
    let id = d.get_element_by_id(name).unwrap();
    d.scroll_to(id, x, y, ScrollBehavior::Instant);
    let offset = d.get_node(id).unwrap().scroll_offset();
    (offset.x, offset.y)
}
fn translated(before: BoundingRect, after: BoundingRect, dx: f64, dy: f64) {
    for (got, expected) in [
        (after.x, before.x - dx),
        (after.y, before.y - dy),
        (after.width, before.width),
        (after.height, before.height),
    ] {
        assert!(
            (got - expected).abs() < 0.04,
            "before={before:?}, after={after:?}, scroll=({dx},{dy}), got={got}, expected={expected}"
        );
    }
}
const PANEL: &str = "<style>body{margin:0;height:1600px}#port{margin:40px;width:240px;height:140px;border:4px solid;overflow:scroll;position:relative}#content{width:800px;height:900px}#leaf{width:30px;height:20px}</style><div id=port aria-label='Scrollable task panel'><div id=content><div id=leaf></div></div></div>";

#[test]
fn own_scroll_keeps_border_box_and_client_rects_in_place() {
    for scale in [1.0, 1.25, 1.5, 2.0, 2.625, 3.0] {
        let mut d = doc(PANEL, scale);
        let port = rect(&d, "port");
        let child = rect(&d, "leaf");
        let (x, y) = scroll(&mut d, "port", 37.5, 63.25);
        assert!(x > 30.0 && y > 60.0);
        translated(port, rect(&d, "port"), 0.0, 0.0);
        translated(child, rect(&d, "leaf"), x, y);
        let id = d.get_element_by_id("port").unwrap();
        let fragments = d.node_client_rects(id);
        assert_eq!(fragments.len(), 1);
        translated(port, fragments[0], 0.0, 0.0);
    }
}
#[test]
fn nested_scrollports_move_only_with_their_ancestors() {
    let mut d = doc(
        "<style>body{margin:0}#outer{margin:30px;width:260px;height:190px;overflow:scroll}#flow{width:900px;height:1100px;padding:20px}#inner{width:180px;height:120px;overflow:scroll}#wide{width:600px;height:800px}#leaf{width:30px;height:20px}</style><div id=outer><div id=flow><div id=inner><div id=wide><div id=leaf></div></div></div></div></div>",
        1.0,
    );
    let outer = rect(&d, "outer");
    let inner = rect(&d, "inner");
    let leaf = rect(&d, "leaf");
    let (ix, iy) = scroll(&mut d, "inner", 17.0, 41.0);
    let (ox, oy) = scroll(&mut d, "outer", 23.0, 52.0);
    assert!(ix > 0.0 && iy > 0.0 && ox > 0.0 && oy > 0.0);
    translated(outer, rect(&d, "outer"), 0.0, 0.0);
    translated(inner, rect(&d, "inner"), ox, oy);
    translated(leaf, rect(&d, "leaf"), ox + ix, oy + iy);
}
#[test]
fn viewport_scroll_still_moves_the_panel_border_box() {
    let mut d = doc(PANEL, 1.0);
    let before = rect(&d, "port");
    scroll(&mut d, "port", 13.0, 29.0);
    let root = d.root_element().id;
    d.scroll_to(root, 0.0, 120.0, ScrollBehavior::Instant);
    assert_eq!(d.viewport_scroll().y, 120.0);
    translated(before, rect(&d, "port"), 0.0, 120.0);
}
#[test]
fn accessibility_panel_bounds_do_not_follow_its_contents() {
    let mut d = doc(PANEL, 1.0);
    let expected = rect(&d, "port");
    scroll(&mut d, "port", 37.0, 64.0);
    let id = accesskit::NodeId(d.get_element_by_id("port").unwrap().as_u64());
    let tree = d.build_accessibility_tree();
    let node = &tree.nodes.iter().find(|(n, _)| *n == id).unwrap().1;
    let r = node.bounds().expect("exposed panel must have bounds");
    assert_eq!(
        (r.x0, r.y0, r.x1, r.y1),
        (
            expected.x,
            expected.y,
            expected.x + expected.width,
            expected.y + expected.height
        )
    );
}
fn inline_doc() -> HtmlDocument {
    doc(
        "<style>body{margin:0}#port{margin:30px;width:110px;height:45px;overflow:scroll;position:relative;font:16px/20px monospace}#text{width:100px}</style><div id=port><span id=text>aaaa aaaa bbbb bbbb cccc cccc dddd dddd eeee eeee ffff ffff gggg gggg</span></div>",
        1.0,
    )
}
#[test]
fn inline_content_fragments_keep_scrolling_with_their_inline_root() {
    let mut d = inline_doc();
    let id = d.get_element_by_id("text").unwrap();
    let before = d.node_client_rects(id);
    assert!(before.len() >= 3);
    let (x, y) = scroll(&mut d, "port", 0.0, 35.0);
    assert!(y > 30.0);
    let after = d.node_client_rects(id);
    assert_eq!(before.len(), after.len());
    for (before, after) in before.into_iter().zip(after) {
        translated(before, after, x, y);
    }
}
#[test]
fn inline_offset_rect_does_not_change_when_offset_parent_scrolls() {
    let mut d = inline_doc();
    let id = d.get_element_by_id("text").unwrap();
    let before = d.offset_rect(id).unwrap();
    let (_, y) = scroll(&mut d, "port", 0.0, 35.0);
    assert!(y > 30.0);
    translated(before, d.offset_rect(id).unwrap(), 0.0, 0.0);
}
#[test]
fn vertical_writing_panels_keep_their_box_while_contents_scroll() {
    for mode in ["vertical-lr", "vertical-rl"] {
        let html = format!(
            "<style>body{{margin:0}}#port{{margin:40px;width:240px;height:140px;overflow:scroll;writing-mode:{mode}}}#content{{width:800px;height:900px}}#leaf{{width:30px;height:20px}}</style><div id=port><div id=content><div id=leaf></div></div></div>"
        );
        let mut d = doc(&html, 1.25);
        let port = rect(&d, "port");
        let leaf = rect(&d, "leaf");
        let (x, y) = scroll(&mut d, "port", 37.0, 63.0);
        assert!(y > 60.0);
        translated(port, rect(&d, "port"), 0.0, 0.0);
        translated(leaf, rect(&d, "leaf"), x, y);
    }
}
