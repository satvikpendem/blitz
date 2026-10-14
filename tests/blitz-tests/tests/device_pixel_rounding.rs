//! Thin borders and absolute positioning must survive fractional display scales.

use blitz_dom::DocumentConfig;
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::shell::{ColorScheme, Viewport};
use std::sync::Arc;

fn document(scale: f32) -> HtmlDocument {
    HtmlDocument::from_html(
        r#"<style>
        body { margin:0; }
        #parent { position:relative; left:16.25px; top:16.75px;
          width:210.3px; height:150.2px; border:1px solid; padding:3.25px; }
        .box { width:82.41905px; height:40.514286px; box-sizing:border-box;
          border:1px solid; border-radius:8px; }
        #flow { margin-left:0.35px; }
        #hoisted { position:absolute; left:102.22857px; top:48.514286px; }
        #intermediate { margin:0.2px; }
        #fixed { position:fixed; left:300.35px; top:100.65px; }
        </style><div id="parent"><div id="flow" class="box"></div>
        <div id="intermediate"><div id="hoisted" class="box"></div></div></div>
        <div id="fixed" class="box"></div>"#,
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, scale, ColorScheme::Light)),
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
            ..Default::default()
        },
    )
}

#[test]
fn nonzero_borders_survive_fractional_display_scales() {
    for scale in [1.0, 1.25, 1.5, 2.0, 2.625, 3.0] {
        let mut doc = document(scale);
        doc.resolve(0.0);
        for selector in ["#parent", "#flow", "#hoisted", "#fixed"] {
            let node = doc
                .get_node(doc.query_selector(selector).unwrap().unwrap())
                .unwrap();
            let layout = node.final_layout();
            let b = layout.border;
            for width in [b.left, b.right, b.top, b.bottom] {
                let pixels = width * scale;
                assert!(
                    pixels >= 0.999,
                    "{selector} at {scale}: missing border: {b:?}"
                );
                assert!(
                    (pixels - pixels.round()).abs() < 0.001,
                    "{selector} at {scale}: border off device grid: {b:?}"
                );
            }
            let original = node.unrounded_layout().border;
            for (rounded, source) in [
                (b.left, original.left),
                (b.right, original.right),
                (b.top, original.top),
                (b.bottom, original.bottom),
            ] {
                assert!(
                    ((rounded - source) * scale).abs() < 1.001,
                    "{selector} at {scale}: rounding changed thickness by over a device pixel"
                );
            }
        }
    }
}

#[test]
fn outer_edges_are_snapped_to_device_pixels_without_rounding_cached_layout() {
    for scale in [1.0, 1.25, 1.5, 2.0, 2.625, 3.0] {
        let mut doc = document(scale);
        doc.resolve(0.0);
        let id = doc.query_selector("#flow").unwrap().unwrap();
        let unrounded = *doc.get_node(id).unwrap().unrounded_layout();
        for selector in ["#parent", "#flow", "#hoisted", "#fixed"] {
            let node = doc
                .get_node(doc.query_selector(selector).unwrap().unwrap())
                .unwrap();
            let layout = node.final_layout();
            for edge in [
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
            ] {
                let pixels = edge * scale;
                assert!(
                    (pixels - pixels.round()).abs() < 0.001,
                    "{selector} at {scale}: {pixels} is off the device grid"
                );
            }
        }
        doc.resolve(1.0);
        assert_eq!(
            *doc.get_node(id).unwrap().unrounded_layout(),
            unrounded,
            "rounding must leave cached CSS layout untouched"
        );
    }
}
