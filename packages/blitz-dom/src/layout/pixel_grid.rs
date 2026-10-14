//! Run Taffy's rounding pass on the device pixel grid, retaining CSS pixels in the DOM.

use taffy::{Layout, NodeId, RoundTree, TraversePartialTree, TraverseTree};

pub(crate) fn round_layout(tree: &mut impl RoundTree, root: NodeId, scale: f32) {
    taffy::round_layout(&mut PixelGrid { tree, scale }, root);
}

struct PixelGrid<'a, T> {
    tree: &'a mut T,
    scale: f32,
}

// Taffy rounds absolute outer and inner edges, including hoisted out-of-flow boxes.
// Scaling at this boundary preserves that traversal without rounding CSS pixels
// first: at fractional display scales that could erase a nonzero border edge.
fn scale_layout(mut layout: Layout, scale: f32) -> Layout {
    layout.location = layout.location.map(|v| v * scale);
    layout.size = layout.size.map(|v| v * scale);
    layout.scrollable_overflow_rect = layout.scrollable_overflow_rect.map(|v| v * scale);
    layout.scrollbar_size = layout.scrollbar_size.map(|v| v * scale);
    layout.border = layout.border.map(|v| v * scale);
    layout.padding = layout.padding.map(|v| v * scale);
    layout
}

impl<T: RoundTree> TraversePartialTree for PixelGrid<'_, T> {
    type ChildIter<'a>
        = T::ChildIter<'a>
    where
        Self: 'a;

    fn child_ids(&self, node: NodeId) -> Self::ChildIter<'_> {
        self.tree.child_ids(node)
    }

    fn child_count(&self, node: NodeId) -> usize {
        self.tree.child_count(node)
    }

    fn get_child_id(&self, node: NodeId, index: usize) -> NodeId {
        self.tree.get_child_id(node, index)
    }
}

impl<T: RoundTree> TraverseTree for PixelGrid<'_, T> {}

impl<T: RoundTree> RoundTree for PixelGrid<'_, T> {
    fn get_unrounded_layout(&self, node: NodeId) -> Layout {
        scale_layout(self.tree.get_unrounded_layout(node), self.scale)
    }

    fn set_final_layout(&mut self, node: NodeId, layout: &Layout) {
        self.tree
            .set_final_layout(node, &scale_layout(*layout, self.scale.recip()));
    }

    fn is_out_of_flow(&self, node: NodeId) -> bool {
        self.tree.is_out_of_flow(node)
    }

    fn hoisted_child_count(&self, node: NodeId) -> usize {
        self.tree.hoisted_child_count(node)
    }

    fn get_hoisted_child_id(&self, node: NodeId, index: usize) -> NodeId {
        self.tree.get_hoisted_child_id(node, index)
    }
}
