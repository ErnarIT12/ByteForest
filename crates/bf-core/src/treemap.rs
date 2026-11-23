use std::collections::VecDeque;

use serde::Serialize;

use crate::category::{FileCategory, category_of};
use crate::flags::NodeFlags;
use crate::tree::{NodeId, Tree};

/// One positioned rectangle of a treemap, in the pixel space given by [`TreemapOptions`].
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Rect {
    /// Node index (`NodeId.0`).
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Distance from the treemap root (the root itself is 0); saturates at 255.
    pub depth: u8,
    pub is_dir: bool,
    /// Category of a file; `None` for directories.
    pub category: Option<FileCategory>,
}

/// Parameters of [`Tree::treemap`].
#[derive(Clone, Debug)]
pub struct TreemapOptions {
    /// Canvas width in pixels.
    pub width: f32,
    /// Canvas height in pixels.
    pub height: f32,
    /// Rectangles narrower or shorter than this are dropped together with their subtree.
    pub min_px: f32,
    /// Upper bound on the number of rectangles returned.
    pub max_rects: usize,
    /// Margin between a directory's border and its children, on every side.
    pub dir_padding: f32,
    /// Height of a name strip reserved at the top of every non-root directory that is at least
    /// three strips wide and three strips tall (see [`header_fits`]). `0` disables it.
    pub dir_header: f32,
}

/// Whether a non-root directory rectangle of `w` x `h` gets a name strip of height `header`.
pub fn header_fits(w: f32, h: f32, header: f32) -> bool {
    header > 0.0 && w >= 3.0 * header && h >= 3.0 * header
}

impl Default for TreemapOptions {
    /// Zero-sized canvas (the caller must set `width` and `height`), `min_px = 3`,
    /// `max_rects = 20_000`, `dir_padding = 1`, `dir_header = 0`.
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            min_px: 3.0,
            max_rects: 20_000,
            dir_padding: 1.0,
            dir_header: 0.0,
        }
    }
}

/// Axis-aligned rectangle used during layout; `f64` keeps rounding errors small.
#[derive(Clone, Copy, Debug)]
struct FRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Tree {
    /// Lays out the subtree under `root` as a squarified treemap.
    ///
    /// The first rectangle is `root` itself covering the whole canvas (`depth = 0`). Children are
    /// sized by their logical subtree size and placed inside their directory's rectangle shrunk
    /// by `dir_padding` (and, for non-root directories, by a `dir_header` strip at the top when
    /// [`header_fits`]). Rectangles come out in breadth-first order, so parents precede
    /// children. Empty (size 0) nodes get no rectangle. Rectangles smaller than `min_px` are
    /// dropped along with their descendants, and output stops at `max_rects`.
    ///
    /// Returns an empty list for a non-positive or non-finite canvas.
    pub fn treemap(&self, root: NodeId, opts: &TreemapOptions) -> Vec<Rect> {
        let canvas = FRect {
            x: 0.0,
            y: 0.0,
            w: f64::from(opts.width),
            h: f64::from(opts.height),
        };
        let min_px = f64::from(opts.min_px);
        let padding = f64::from(opts.dir_padding).max(0.0);
        let header = if opts.dir_header.is_finite() {
            opts.dir_header.max(0.0)
        } else {
            0.0
        };
        if !(canvas.w.is_finite() && canvas.h.is_finite() && canvas.w > 0.0 && canvas.h > 0.0) {
            return Vec::new();
        }
        if canvas.w < min_px || canvas.h < min_px {
            return Vec::new();
        }

        let mut rects = Vec::new();
        // Explicit queue instead of recursion: trees can be 100k levels deep.
        let mut queue = VecDeque::new();
        queue.push_back((root, canvas, 0u8));
        while let Some((id, area, depth)) = queue.pop_front() {
            if rects.len() >= opts.max_rects {
                break;
            }
            let flags = self.flags(id);
            let is_dir = flags.contains(NodeFlags::DIR);
            rects.push(Rect {
                id: id.0,
                x: area.x as f32,
                y: area.y as f32,
                w: area.w as f32,
                h: area.h as f32,
                depth,
                is_dir,
                category: (!is_dir).then(|| category_of(self.name(id), flags)),
            });
            if !is_dir {
                continue;
            }

            let top = if depth > 0 && header_fits(area.w as f32, area.h as f32, header) {
                f64::from(header)
            } else {
                0.0
            };
            let inner = FRect {
                x: area.x + padding,
                y: area.y + padding + top,
                w: area.w - 2.0 * padding,
                h: area.h - 2.0 * padding - top,
            };
            if inner.w <= 0.0 || inner.h <= 0.0 {
                continue;
            }
            let mut items: Vec<(u32, f64)> = self
                .children_ids(id)
                .filter_map(|c| {
                    let size = self.total(c).size;
                    (size > 0).then_some((c.0, size as f64))
                })
                .collect();
            // Largest first; equal sizes ordered by id so the layout is deterministic.
            items.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            let child_depth = depth.saturating_add(1);
            for (child, r) in squarify(&items, inner) {
                if r.w >= min_px && r.h >= min_px {
                    queue.push_back((NodeId(child), r, child_depth));
                }
            }
        }
        rects
    }
}

/// Worst (largest) aspect ratio of a row with total area `sum`, smallest item `min` and
/// largest item `max`, laid along a side of length `side`.
fn worst_ratio(sum: f64, min: f64, max: f64, side: f64) -> f64 {
    let side2 = side * side;
    let sum2 = sum * sum;
    (side2 * max / sum2).max(sum2 / (side2 * min))
}

/// Squarified treemap (Bruls, Huizing, van Wijk 2000) on plain numbers.
///
/// `items` are `(id, weight)` pairs sorted by weight, largest first, all weights positive.
/// The weights are scaled so that together they exactly fill `free`; each returned rectangle's
/// area is proportional to its weight. Rows run along the shorter side of the free space and
/// grow while the worst aspect ratio in the row does not get worse.
fn squarify(items: &[(u32, f64)], free: FRect) -> Vec<(u32, FRect)> {
    let total: f64 = items.iter().map(|&(_, w)| w).sum();
    if items.is_empty() || total <= 0.0 || free.w <= 0.0 || free.h <= 0.0 {
        return Vec::new();
    }
    let scale = free.w * free.h / total;
    let areas: Vec<f64> = items.iter().map(|&(_, w)| w * scale).collect();

    let mut out = Vec::with_capacity(items.len());
    let mut rest = free;
    let mut start = 0;
    while start < items.len() {
        // The row lies along the shorter side; its thickness is cut from the other side.
        let side = rest.w.min(rest.h);

        let mut end = start + 1;
        let mut row_sum = areas[start];
        let mut worst = worst_ratio(row_sum, areas[start], areas[start], side);
        while end < items.len() {
            let next_sum = row_sum + areas[end];
            // Sorted descending: the first item is the largest, the new one the smallest.
            let next_worst = worst_ratio(next_sum, areas[end], areas[start], side);
            if next_worst > worst {
                break;
            }
            row_sum = next_sum;
            worst = next_worst;
            end += 1;
        }

        // Final row takes whatever is left, so rounding never leaves a sliver uncovered.
        let wide = rest.w >= rest.h;
        let thickness = if end == items.len() {
            if wide { rest.w } else { rest.h }
        } else if wide {
            row_sum / rest.h
        } else {
            row_sum / rest.w
        };

        let mut offset = 0.0;
        for i in start..end {
            let length = if i + 1 == end {
                side - offset
            } else {
                areas[i] / row_sum * side
            };
            let r = if wide {
                FRect {
                    x: rest.x,
                    y: rest.y + offset,
                    w: thickness,
                    h: length,
                }
            } else {
                FRect {
                    x: rest.x + offset,
                    y: rest.y,
                    w: length,
                    h: thickness,
                }
            };
            out.push((items[i].0, r));
            offset += length;
        }

        if wide {
            rest.x += thickness;
            rest.w -= thickness;
        } else {
            rest.y += thickness;
            rest.h -= thickness;
        }
        start = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_square() -> FRect {
        FRect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        }
    }

    fn area(r: &FRect) -> f64 {
        r.w * r.h
    }

    #[test]
    fn empty_input_gives_no_rects() {
        assert!(squarify(&[], unit_square()).is_empty());
    }

    #[test]
    fn single_item_fills_the_free_rect() {
        let out = squarify(&[(7, 123.0)], unit_square());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 7);
        assert!((area(&out[0].1) - 10_000.0).abs() < 1e-9);
    }

    #[test]
    fn two_equal_items_split_the_square_in_half() {
        let out = squarify(&[(1, 5.0), (2, 5.0)], unit_square());
        assert_eq!(out.len(), 2);
        for (_, r) in &out {
            assert!((area(r) - 5000.0).abs() < 1e-6);
            assert!((r.w.max(r.h) / r.w.min(r.h) - 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn paper_example_from_bruls_et_al() {
        // Weights 6,6,4,3,2,2,1 in a 6x4 rectangle (the example from the paper).
        let free = FRect {
            x: 0.0,
            y: 0.0,
            w: 6.0,
            h: 4.0,
        };
        let items: Vec<(u32, f64)> = [6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0]
            .iter()
            .enumerate()
            .map(|(i, &w)| (i as u32, w))
            .collect();
        let out = squarify(&items, free);
        assert_eq!(out.len(), 7);
        // The first row is the 6 and 6 stacked in the left column, each 3 wide and 2 tall.
        for (i, (_, r)) in out.iter().take(2).enumerate() {
            assert!(
                (r.x - 0.0).abs() < 1e-9 && (r.w - 3.0).abs() < 1e-9,
                "{r:?}"
            );
            assert!((r.y - 2.0 * i as f64).abs() < 1e-9 && (r.h - 2.0).abs() < 1e-9);
        }
        for ((_, r), w) in out.iter().zip([6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0]) {
            assert!((area(r) - w).abs() < 1e-9, "{r:?} should have area {w}");
        }
    }

    #[test]
    fn rects_tile_the_free_rect_without_gaps() {
        let free = FRect {
            x: 10.0,
            y: 20.0,
            w: 80.0,
            h: 30.0,
        };
        let items: Vec<(u32, f64)> = (0..37).map(|i| (i, 100.0 - f64::from(i))).collect();
        let out = squarify(&items, free);
        let covered: f64 = out.iter().map(|(_, r)| area(r)).sum();
        assert!((covered - area(&free)).abs() < 1e-6);
        for (_, r) in &out {
            assert!(r.x >= free.x - 1e-9 && r.y >= free.y - 1e-9);
            assert!(r.x + r.w <= free.x + free.w + 1e-9);
            assert!(r.y + r.h <= free.y + free.h + 1e-9);
        }
    }
}
