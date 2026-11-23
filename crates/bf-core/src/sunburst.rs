use std::collections::VecDeque;
use std::f64::consts::TAU;

use serde::Serialize;

use crate::category::{FileCategory, category_of};
use crate::flags::NodeFlags;
use crate::tree::{NodeId, Tree};

/// Id of the synthetic arc that stands for the small children merged by [`Tree::sunburst`].
pub const OTHER_ARC_ID: u32 = u32::MAX;

/// Deepest ring [`SunburstOptions::max_depth`] can ask for.
pub const MAX_SUNBURST_DEPTH: u8 = 12;

/// One ring segment of a sunburst, in radians measured clockwise from 12 o'clock.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SunburstArc {
    /// Node index (`NodeId.0`), or [`OTHER_ARC_ID`] for a merged "other" arc.
    pub id: u32,
    /// Node index of the parent arc's node (the focused node for the first ring).
    pub parent: u32,
    /// Ring number: 1 is the ring around the centre disc.
    pub depth: u8,
    /// Start angle in `[0, TAU]`.
    pub start: f32,
    /// End angle in `[0, TAU]`, never before `start`.
    pub end: f32,
    pub is_dir: bool,
    /// Category of a file; `None` for directories and the "other" arc.
    pub category: Option<FileCategory>,
}

/// Parameters of [`Tree::sunburst`].
#[derive(Clone, Debug)]
pub struct SunburstOptions {
    /// Rings to produce, clamped to `1..=12`.
    pub max_depth: u8,
    /// Children narrower than this many radians are merged into one "other" arc per parent.
    pub min_angle: f32,
    /// Upper bound on the number of arcs returned.
    pub max_segments: usize,
}

impl Default for SunburstOptions {
    /// `max_depth = 6`, `min_angle = 0.004`, `max_segments = 20_000`.
    fn default() -> Self {
        Self {
            max_depth: 6,
            min_angle: 0.004,
            max_segments: 20_000,
        }
    }
}

impl Tree {
    /// Lays out the subtree under `root` as a sunburst (radial treemap).
    ///
    /// The children of `root` fill the full circle; every directory's children split that
    /// directory's arc in proportion to their logical subtree size, largest first (equal sizes
    /// ordered by id). Directory arcs keep the size recorded on the directory itself as an
    /// unused tail, so children never fill more than their parent.
    ///
    /// Children narrower than `min_angle` are merged into one arc per parent with
    /// `id = OTHER_ARC_ID`, placed last. If even the merged arc is narrower than `min_angle`
    /// it is dropped. Merged arcs are never descended into. Empty (size 0) nodes get no arc.
    ///
    /// Arcs come out ring by ring (breadth first), so output cut at `max_segments` always holds
    /// the inner rings completely. Nothing is produced beyond `max_depth` rings.
    ///
    /// Returns an empty list for an unknown or removed `root`, or one without any size.
    pub fn sunburst(&self, root: NodeId, opts: &SunburstOptions) -> Vec<SunburstArc> {
        let mut arcs = Vec::new();
        if root.idx() >= self.len() || self.is_removed(root) {
            return arcs;
        }
        let max_depth = opts.max_depth.clamp(1, MAX_SUNBURST_DEPTH);
        let min_angle = if opts.min_angle.is_finite() {
            f64::from(opts.min_angle).max(0.0)
        } else {
            0.0
        };

        // (node, start, end, depth of its children)
        let mut queue = VecDeque::new();
        queue.push_back((root, 0.0_f64, TAU, 1u8));
        while let Some((id, start, end, depth)) = queue.pop_front() {
            if arcs.len() >= opts.max_segments {
                break;
            }
            if !self.flags(id).contains(NodeFlags::DIR) {
                continue;
            }
            let total = self.total(id).size as f64;
            if total <= 0.0 || end <= start {
                continue;
            }
            let mut items: Vec<(u32, f64)> = self
                .children_ids(id)
                .filter_map(|c| {
                    let size = self.total(c).size;
                    (size > 0).then_some((c.0, size as f64))
                })
                .collect();
            items.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));

            let span = end - start;
            let mut cum = 0.0_f64;
            let mut iter = items.iter().peekable();
            while let Some(&&(child, size)) = iter.peek() {
                if size / total * span < min_angle {
                    break;
                }
                iter.next();
                if arcs.len() >= opts.max_segments {
                    return arcs;
                }
                let a = (start + cum / total * span).clamp(0.0, TAU);
                cum += size;
                let b = (start + cum / total * span).clamp(a, TAU);
                let cid = NodeId(child);
                let flags = self.flags(cid);
                let is_dir = flags.contains(NodeFlags::DIR);
                arcs.push(SunburstArc {
                    id: child,
                    parent: id.0,
                    depth,
                    start: a as f32,
                    end: b as f32,
                    is_dir,
                    category: (!is_dir).then(|| category_of(self.name(cid), flags)),
                });
                if is_dir && depth < max_depth {
                    queue.push_back((cid, a, b, depth + 1));
                }
            }
            // Everything left is narrower than `min_angle`: one shared arc, or nothing.
            let rest: f64 = iter.map(|&(_, s)| s).sum();
            let width = rest / total * span;
            if rest > 0.0 && width >= min_angle && arcs.len() < opts.max_segments {
                let a = (start + cum / total * span).clamp(0.0, TAU);
                let b = (a + width).clamp(a, TAU);
                arcs.push(SunburstArc {
                    id: OTHER_ARC_ID,
                    parent: id.0,
                    depth,
                    start: a as f32,
                    end: b as f32,
                    is_dir: false,
                    category: None,
                });
            }
        }
        arcs
    }
}
