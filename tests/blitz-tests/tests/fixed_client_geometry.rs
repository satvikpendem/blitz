//! Viewport-fixed boxes and their contents keep the same client-space origin.
//! Literal AccessKit consumer bounds keep the oracle independent of CSSOM helpers.
use accesskit::{Affine, NodeId, Rect, TreeId};
use blitz_dom::{BoundingRect, DocumentConfig, Point, ScrollBehavior};
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::shell::{ColorScheme, Viewport};
use std::sync::Arc;

fn doc(content: &str, scale: f32) -> HtmlDocument {
    let mut doc = HtmlDocument::from_html(
        content,
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
    doc.resolve(0.0);
    doc
}

fn rect(doc: &HtmlDocument, name: &str) -> BoundingRect {
    doc.get_client_bounding_rect(doc.get_element_by_id(name).unwrap())
        .unwrap()
}

fn viewport_scroll(doc: &mut HtmlDocument, x: f64, y: f64) {
    let root = doc.root_element().id;
    doc.scroll_to(root, x, y, ScrollBehavior::Instant);
    assert_eq!(doc.viewport_scroll(), Point { x, y });
    // Exercise canonical scroll APIs, not an artificial second root-node offset.
    assert_eq!(
        *doc.get_node(root).unwrap().scroll_offset(),
        Point { x: 0.0, y: 0.0 }
    );
}

fn assert_rect(actual: BoundingRect, expected: [f64; 4]) {
    for (got, want) in [actual.x, actual.y, actual.width, actual.height]
        .into_iter()
        .zip(expected)
    {
        assert!(
            (got - want).abs() < 0.04,
            "{actual:?}, expected {expected:?}"
        );
    }
}

fn consumer_bounds(doc: &HtmlDocument, name: &str, physical_offset: (f64, f64)) -> Rect {
    let mut update = doc.build_accessibility_tree();
    // This models the shell's root transform; it does not prove a native bridge.
    let root_id = update.tree.as_ref().unwrap().root;
    let root = &mut update
        .nodes
        .iter_mut()
        .find(|(id, _)| *id == root_id)
        .unwrap()
        .1;
    let scale = doc.viewport().scale_f64();
    root.set_transform(Affine::new([
        scale,
        0.0,
        0.0,
        scale,
        physical_offset.0,
        physical_offset.1,
    ]));
    let tree = accesskit_consumer::Tree::new(update, true);
    let id = NodeId(doc.get_element_by_id(name).unwrap().as_u64());
    tree.state()
        .node_by_tree_local_id(id, TreeId::ROOT)
        .unwrap()
        .bounding_box()
        .unwrap()
}

const SIMPLE: &str = r#"<style>body{margin:0;width:2000px;height:3000px}button{padding:0;border:0;width:80px;height:30px}#target{position:fixed;left:40px;top:100px}</style><button id=target>Load</button>"#;

#[test]
fn fixed_button_keeps_literal_consumer_bounds_across_viewport_scroll_and_dpr() {
    for scale in [1.0, 2.0, 2.625] {
        let mut doc = doc(SIMPLE, scale);
        for (x, y) in [(0.0, 0.0), (17.5, 50.0), (0.0, 0.0)] {
            viewport_scroll(&mut doc, x, y);
            for offset in [(0.0, 0.0), (13.0, 29.0)] {
                let scale = f64::from(scale);
                assert_eq!(
                    consumer_bounds(&doc, "target", offset),
                    Rect::new(
                        40.0 * scale + offset.0,
                        100.0 * scale + offset.1,
                        120.0 * scale + offset.0,
                        130.0 * scale + offset.1,
                    )
                );
            }
        }
    }
}

#[test]
fn fixed_button_client_rect_and_client_fragments_stay_stationary() {
    let mut doc = doc(SIMPLE, 2.0);
    let target = doc.get_element_by_id("target").unwrap();
    viewport_scroll(&mut doc, 17.5, 50.0);
    assert_rect(rect(&doc, "target"), [40.0, 100.0, 80.0, 30.0]);
    let fragments = doc.node_client_rects(target);
    assert_eq!(fragments.len(), 1);
    assert_rect(fragments[0], [40.0, 100.0, 80.0, 30.0]);
}

#[test]
fn zoomed_fixed_button_keeps_css_geometry_and_physical_root_offset() {
    let mut doc = doc(SIMPLE, 2.0);
    doc.zoom_to(1.25);
    doc.resolve(0.0);
    viewport_scroll(&mut doc, 17.5, 50.0);
    assert_eq!(doc.viewport().scale_f64(), 2.5);
    assert_rect(rect(&doc, "target"), [40.0, 100.0, 80.0, 30.0]);
    assert_eq!(
        consumer_bounds(&doc, "target", (13.0, 29.0)),
        Rect::new(113.0, 279.0, 313.0, 354.0)
    );
}

#[test]
fn fixed_panel_descendant_keeps_literal_client_and_consumer_bounds() {
    let html = r#"<style>body{margin:0;width:2000px;height:3000px}#panel{position:fixed;left:40px;top:100px;width:240px;height:140px}#target{position:absolute;left:12px;top:18px;width:80px;height:30px;padding:0;border:0}</style><div id=panel><button id=target>Load</button></div>"#;
    for scale in [1.0, 2.0, 2.625] {
        let mut doc = doc(html, scale);
        viewport_scroll(&mut doc, 17.5, 50.0);
        assert_rect(rect(&doc, "target"), [52.0, 118.0, 80.0, 30.0]);
        let scale = f64::from(scale);
        assert_eq!(
            consumer_bounds(&doc, "target", (0.0, 0.0)),
            Rect::new(52.0 * scale, 118.0 * scale, 132.0 * scale, 148.0 * scale)
        );
    }
}

#[test]
fn scrolling_inside_fixed_panel_moves_only_its_contents() {
    let html = r#"<style>body{margin:0;width:2000px;height:3000px}#panel{position:fixed;left:40px;top:100px;width:240px;height:140px;overflow:scroll}#content{position:relative;width:800px;height:800px}#target{position:absolute;left:40px;top:60px;width:80px;height:30px;padding:0;border:0}</style><div id=panel><div id=content><button id=target>Load</button></div></div>"#;
    let mut doc = doc(html, 2.0);
    let panel = doc.get_element_by_id("panel").unwrap();
    doc.scroll_to(panel, 13.0, 29.0, ScrollBehavior::Instant);
    assert_eq!(
        *doc.get_node(panel).unwrap().scroll_offset(),
        Point { x: 13.0, y: 29.0 }
    );
    for (x, y) in [(0.0, 0.0), (17.5, 50.0)] {
        viewport_scroll(&mut doc, x, y);
        assert_rect(rect(&doc, "panel"), [40.0, 100.0, 240.0, 140.0]);
        assert_rect(rect(&doc, "target"), [67.0, 131.0, 80.0, 30.0]);
        assert_eq!(
            consumer_bounds(&doc, "target", (0.0, 0.0)),
            Rect::new(134.0, 262.0, 294.0, 322.0)
        );
    }
}

#[test]
fn absolute_flow_control_still_moves_with_viewport_scroll() {
    let mut doc = doc(&SIMPLE.replace("position:fixed", "position:absolute"), 2.0);
    viewport_scroll(&mut doc, 17.5, 50.0);
    assert_rect(rect(&doc, "target"), [22.5, 50.0, 80.0, 30.0]);
    assert_eq!(
        consumer_bounds(&doc, "target", (0.0, 0.0)),
        Rect::new(45.0, 100.0, 205.0, 160.0)
    );
}

#[test]
fn fixed_control_in_flowing_transformed_container_still_moves_with_viewport() {
    let html = r#"<style>body{margin:0;width:2000px;height:3000px}#container{position:absolute;left:40px;top:100px;width:240px;height:140px;transform:translate(0px,0px)}#target{position:fixed;left:12px;top:18px;width:80px;height:30px;padding:0;border:0}</style><div id=container><button id=target>Load</button></div>"#;
    let mut doc = doc(html, 2.0);
    viewport_scroll(&mut doc, 17.5, 50.0);
    assert_rect(rect(&doc, "target"), [34.5, 68.0, 80.0, 30.0]);
    assert_eq!(
        consumer_bounds(&doc, "target", (0.0, 0.0)),
        Rect::new(69.0, 136.0, 229.0, 196.0)
    );
}

#[test]
fn outer_fixed_panel_keeps_transformed_containing_block_fixed_child_stationary() {
    let html = r#"<style>body{margin:0;width:2000px;height:3000px}#panel{position:fixed;left:40px;top:100px;width:240px;height:140px}#container{position:absolute;left:12px;top:18px;width:160px;height:100px;transform:translate(0px,0px)}#target{position:fixed;left:7px;top:9px;width:80px;height:30px;padding:0;border:0}</style><div id=panel><div id=container><button id=target>Load</button></div></div>"#;
    let mut doc = doc(html, 2.0);
    viewport_scroll(&mut doc, 17.5, 50.0);
    assert_rect(rect(&doc, "target"), [59.0, 127.0, 80.0, 30.0]);
    assert_eq!(
        consumer_bounds(&doc, "target", (0.0, 0.0)),
        Rect::new(118.0, 254.0, 278.0, 314.0)
    );
}

fn inline_fixture(position: &str) -> String {
    format!(
        r#"<style>body{{margin:0;width:2000px;height:3000px}}#panel{{position:{position};left:40px;top:100px;width:130px;font-size:20px;line-height:24px}}</style><div id=panel><span id=target>one two three four five six seven eight nine ten</span></div>"#
    )
}

#[test]
fn fixed_inline_fragments_and_offset_rect_do_not_drift() {
    for scale in [1.0, 2.0, 2.625] {
        let mut doc = doc(&inline_fixture("fixed"), scale);
        let target = doc.get_element_by_id("target").unwrap();
        let fragments = doc.node_client_rects(target);
        assert!(
            fragments.len() >= 2,
            "fixture must exercise multiple inline fragments"
        );
        let bounds = rect(&doc, "target");
        let offset = doc.offset_rect(target).unwrap();
        viewport_scroll(&mut doc, 17.5, 50.0);
        assert_eq!(doc.node_client_rects(target), fragments);
        assert_eq!(rect(&doc, "target"), bounds);
        assert_eq!(doc.offset_rect(target).unwrap(), offset);
    }
}

#[test]
fn flowing_inline_fragments_move_while_offset_rect_stays_stable() {
    let mut doc = doc(&inline_fixture("absolute"), 2.0);
    let target = doc.get_element_by_id("target").unwrap();
    let fragments = doc.node_client_rects(target);
    assert!(fragments.len() >= 2);
    let offset = doc.offset_rect(target).unwrap();
    viewport_scroll(&mut doc, 17.5, 50.0);
    let moved = doc.node_client_rects(target);
    assert_eq!(moved.len(), fragments.len());
    for (before, after) in fragments.into_iter().zip(moved) {
        assert_rect(
            after,
            [
                before.x - 17.5,
                before.y - 50.0,
                before.width,
                before.height,
            ],
        );
    }
    assert_eq!(doc.offset_rect(target).unwrap(), offset);
}

#[cfg(feature = "writing-mode")]
#[test]
fn vertical_writing_modes_keep_fixed_physical_client_and_consumer_bounds() {
    for writing_mode in ["vertical-lr", "vertical-rl"] {
        for scale in [1.0, 2.0, 2.625] {
            let html = SIMPLE.replace(
                "body{",
                &format!("html{{writing-mode:{writing_mode}}}body{{"),
            );
            let mut doc = doc(&html, scale);
            assert_rect(rect(&doc, "target"), [40.0, 100.0, 80.0, 30.0]);
            viewport_scroll(&mut doc, 17.5, 50.0);
            assert_rect(rect(&doc, "target"), [40.0, 100.0, 80.0, 30.0]);
            let scale = f64::from(scale);
            assert_eq!(
                consumer_bounds(&doc, "target", (0.0, 0.0)),
                Rect::new(40.0 * scale, 100.0 * scale, 120.0 * scale, 130.0 * scale,)
            );
        }
    }
}
