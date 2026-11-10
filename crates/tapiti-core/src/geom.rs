//! Plane geometry of the dungeon model (`docs/dungeon-model.md`): directions of the alphabet Θ₈ (multiples of 45°), shapes (polygons
//! and discs), the distance between two shapes and the band a corridor segment covers. Only `+ − × ÷`, `sqrt` and the libm that ships
//! inside the WASM module, so it rounds the same in every browser.

use serde::Serialize;

pub type V = [f64; 2];

const S: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// u(k·45°) for k = 0..7, exact (y grows downward, as on screen).
pub const DIRS: [V; 8] = [
    [1.0, 0.0],
    [S, S],
    [0.0, 1.0],
    [-S, S],
    [-1.0, 0.0],
    [-S, -S],
    [0.0, -1.0],
    [S, -S],
];

pub fn dir_of(k: i32) -> V {
    DIRS[(((k % 8) + 8) % 8) as usize]
}

/// The turn between directions `a` and `b`, in steps of 45° (0..4).
pub fn turn_of(a: i32, b: i32) -> i32 {
    let d = (((b - a) % 8) + 8) % 8;
    if d > 4 {
        8 - d
    } else {
        d
    }
}

pub fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1]]
}
pub fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1]]
}
pub fn mul(a: V, s: f64) -> V {
    [a[0] * s, a[1] * s]
}
pub fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
pub fn cross(a: V, b: V) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
pub fn len(a: V) -> f64 {
    (a[0] * a[0] + a[1] * a[1]).sqrt()
}
pub fn dist(a: V, b: V) -> f64 {
    len(sub(a, b))
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Shape {
    Poly { pts: Vec<V> },
    Disc { c: V, r: f64 },
}

/// Axis-aligned bounds for a shape, stored separately when the shape is
/// already owned elsewhere (for example by a room).
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub lo: V,
    pub hi: V,
}

/// A shape with its box, so far-apart shapes are told apart without touching their edges.
#[derive(Clone, Debug, Serialize)]
pub struct Bounded {
    #[serde(flatten)]
    pub shape: Shape,
    #[serde(skip)]
    pub lo: V,
    #[serde(skip)]
    pub hi: V,
}

pub fn bounds_of(s: &Shape) -> (V, V) {
    match s {
        Shape::Disc { c, r } => ([c[0] - r, c[1] - r], [c[0] + r, c[1] + r]),
        Shape::Poly { pts } => {
            let mut lo = [f64::INFINITY; 2];
            let mut hi = [f64::NEG_INFINITY; 2];
            for p in pts {
                lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                hi = [hi[0].max(p[0]), hi[1].max(p[1])];
            }
            (lo, hi)
        }
    }
}

pub fn shape_bounds(shape: &Shape) -> Bounds {
    let (lo, hi) = bounds_of(shape);
    Bounds { lo, hi }
}

pub fn bounded(shape: Shape) -> Bounded {
    let (lo, hi) = bounds_of(&shape);
    Bounded { shape, lo, hi }
}

/// The least distance between two boxes (0 when they overlap): a lower bound of the distance between what they hold.
fn box_gap(a: &Bounded, b: &Bounded) -> f64 {
    box_gap_bounds(Bounds { lo: a.lo, hi: a.hi }, Bounds { lo: b.lo, hi: b.hi })
}

fn box_gap_bounds(a: Bounds, b: Bounds) -> f64 {
    let dx = (a.lo[0] - b.hi[0]).max(b.lo[0] - a.hi[0]).max(0.0);
    let dy = (a.lo[1] - b.hi[1]).max(b.lo[1] - a.hi[1]).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// What "touching" means to `*_within`: a distance below it is a distance of 0.
pub const TOUCH: f64 = f64::MIN_POSITIVE;

/// The gap between the boxes of segments a-b and c-d: never more than the distance between the segments.
fn seg_box_gap(a: V, b: V, c: V, d: V) -> f64 {
    let dx = (a[0].min(b[0]) - c[0].max(d[0]))
        .max(c[0].min(d[0]) - a[0].max(b[0]))
        .max(0.0);
    let dy = (a[1].min(b[1]) - c[1].max(d[1]))
        .max(c[1].min(d[1]) - a[1].max(b[1]))
        .max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// Twice the signed area (shoelace).
pub fn signed_area2(pts: &[V]) -> f64 {
    let mut a = 0.0;
    let n = pts.len();
    let mut j = n - 1;
    for i in 0..n {
        a += pts[j][0] * pts[i][1] - pts[i][0] * pts[j][1];
        j = i;
    }
    a
}

pub fn area_of(s: &Shape) -> f64 {
    match s {
        Shape::Disc { r, .. } => std::f64::consts::PI * r * r,
        Shape::Poly { pts } => signed_area2(pts).abs() / 2.0,
    }
}

pub fn centroid(s: &Shape) -> V {
    match s {
        Shape::Disc { c, .. } => *c,
        Shape::Poly { pts } => {
            let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
            let n = pts.len();
            let mut j = n - 1;
            for i in 0..n {
                let f = pts[j][0] * pts[i][1] - pts[i][0] * pts[j][1];
                a += f;
                cx += (pts[j][0] + pts[i][0]) * f;
                cy += (pts[j][1] + pts[i][1]) * f;
                j = i;
            }
            [cx / (3.0 * a), cy / (3.0 * a)]
        }
    }
}

/// Even-odd rule.
pub fn inside_poly(pts: &[V], p: V) -> bool {
    let mut inside = false;
    let n = pts.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (pts[i][0], pts[i][1]);
        let (xj, yj) = (pts[j][0], pts[j][1]);
        if (yi > p[1]) != (yj > p[1]) && p[0] < (xj - xi) * (p[1] - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Distance from `p` to the segment a-b.
pub fn seg_point(p: V, a: V, b: V) -> f64 {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    let t = if l2 > 0.0 {
        (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    dist(p, add(a, mul(ab, t)))
}

/// Whether segments a-b and c-d cross or touch.
pub fn segs_cross(a: V, b: V, c: V, d: V) -> bool {
    let d1 = cross(sub(b, a), sub(c, a));
    let d2 = cross(sub(b, a), sub(d, a));
    let d3 = cross(sub(d, c), sub(a, c));
    let d4 = cross(sub(d, c), sub(b, c));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    (d1 == 0.0 && seg_point(c, a, b) == 0.0)
        || (d2 == 0.0 && seg_point(d, a, b) == 0.0)
        || (d3 == 0.0 && seg_point(a, c, d) == 0.0)
        || (d4 == 0.0 && seg_point(b, c, d) == 0.0)
}

fn seg_seg(a: V, b: V, c: V, d: V) -> f64 {
    if segs_cross(a, b, c, d) {
        0.0
    } else {
        seg_point(a, c, d)
            .min(seg_point(b, c, d))
            .min(seg_point(c, a, b))
            .min(seg_point(d, a, b))
    }
}

/// Distance from a point to a shape (0 inside it).
fn point_shape(p: V, s: &Shape) -> f64 {
    match s {
        Shape::Disc { c, r } => (dist(p, *c) - r).max(0.0),
        Shape::Poly { pts } => {
            if inside_poly(pts, p) {
                return 0.0;
            }
            let n = pts.len();
            let mut best = f64::INFINITY;
            for i in 0..n {
                best = best.min(seg_point(p, pts[i], pts[(i + 1) % n]));
            }
            best
        }
    }
}

/// The least distance between two shapes, 0 when they touch or overlap.
pub fn shape_dist(a: &Shape, b: &Shape) -> f64 {
    match (a, b) {
        (Shape::Disc { c: ca, r: ra }, Shape::Disc { c: cb, r: rb }) => {
            (dist(*ca, *cb) - ra - rb).max(0.0)
        }
        (Shape::Disc { c, r }, other) | (other, Shape::Disc { c, r }) => {
            (point_shape(*c, other) - r).max(0.0)
        }
        (Shape::Poly { pts: pa }, Shape::Poly { pts: pb }) => {
            if inside_poly(pa, pb[0]) || inside_poly(pb, pa[0]) {
                return 0.0;
            }
            let (na, nb) = (pa.len(), pb.len());
            let mut best = f64::INFINITY;
            for i in 0..na {
                let (p, q) = (pa[i], pa[(i + 1) % na]);
                for j in 0..nb {
                    best = best.min(seg_seg(p, q, pb[j], pb[(j + 1) % nb]));
                }
            }
            best
        }
    }
}

/// Whether the distance between two shapes is less than `limit`: the same answer as `shape_dist(a, b) < limit`, without working the distance out.
/// Pairs of edges whose boxes are `limit` apart are skipped, and the first pair closer than it ends the search.
pub fn shape_within(a: &Shape, b: &Shape, limit: f64) -> bool {
    // Distances are nonnegative, so a nonpositive (or NaN) bound can never
    // satisfy the strict comparison `shape_dist(a, b) < limit`.
    if !(limit > 0.0) {
        return false;
    }
    match (a, b) {
        (Shape::Disc { c: ca, r: ra }, Shape::Disc { c: cb, r: rb }) => {
            (dist(*ca, *cb) - ra - rb).max(0.0) < limit
        }
        (Shape::Disc { c, r }, other) | (other, Shape::Disc { c, r }) => {
            (point_shape(*c, other) - r).max(0.0) < limit
        }
        (Shape::Poly { pts: pa }, Shape::Poly { pts: pb }) => {
            if inside_poly(pa, pb[0]) || inside_poly(pb, pa[0]) {
                return true;
            }
            let (na, nb) = (pa.len(), pb.len());
            for i in 0..na {
                let (p, q) = (pa[i], pa[(i + 1) % na]);
                for j in 0..nb {
                    let (r, s) = (pb[j], pb[(j + 1) % nb]);
                    if seg_box_gap(p, q, r, s) >= limit {
                        continue;
                    }
                    if seg_seg(p, q, r, s) < limit {
                        return true;
                    }
                }
            }
            false
        }
    }
}

/// Whether some shape of `a` is less than `limit` from some shape of `b` (`region_dist(a, b) < limit`, without the distance).
pub fn region_within(a: &[Bounded], b: &[Bounded], limit: f64) -> bool {
    for x in a {
        for y in b {
            if box_gap(x, y) >= limit {
                continue;
            }
            if shape_within(&x.shape, &y.shape, limit) {
                return true;
            }
        }
    }
    false
}

/// Distance between a list of owned shapes and one shape whose bounds are
/// stored separately. This lets `Room` keep one copy of its polygon.
pub fn region_shape_dist(region: &[Bounded], shape: &Shape, bounds: Bounds) -> f64 {
    let mut best = f64::INFINITY;
    for part in region {
        if box_gap_bounds(
            Bounds {
                lo: part.lo,
                hi: part.hi,
            },
            bounds,
        ) >= best
        {
            continue;
        }
        best = best.min(shape_dist(&part.shape, shape));
        if best == 0.0 {
            return 0.0;
        }
    }
    best
}

pub fn region_shape_within(region: &[Bounded], shape: &Shape, bounds: Bounds, limit: f64) -> bool {
    region_shape_dist(region, shape, bounds) < limit
}

/// The rectangle of width `w` along the segment a-b (no caps).
pub fn band(a: V, b: V, w: f64) -> Shape {
    let l = dist(a, b);
    let n: V = if l > 0.0 {
        [-(b[1] - a[1]) / l, (b[0] - a[0]) / l]
    } else {
        [0.0, 0.0]
    };
    let h = mul(n, w / 2.0);
    Shape::Poly {
        pts: vec![add(a, h), add(b, h), sub(b, h), sub(a, h)],
    }
}

/// A regular octagon with edges facing the 8 directions, `w` across: it fills the joint of two bands meeting at 45° or 90°.
pub fn joint(c: V, w: f64) -> Shape {
    let r = w / 2.0 / (std::f64::consts::PI / 8.0).cos();
    let pts = (0..8)
        .map(|i| {
            let a = std::f64::consts::PI / 8.0 + i as f64 * std::f64::consts::PI / 4.0;
            add(c, mul([a.cos(), a.sin()], r))
        })
        .collect();
    Shape::Poly { pts }
}

/// The region of a polyline corridor of width `w`: a band per segment and a joint at each inner vertex.
pub fn corridor_region(pts: &[V], w: f64) -> Vec<Bounded> {
    let mut out: Vec<Bounded> = Vec::with_capacity(pts.len() * 2);
    for i in 1..pts.len() {
        out.push(bounded(band(pts[i - 1], pts[i], w)));
    }
    for p in pts.iter().take(pts.len().saturating_sub(1)).skip(1) {
        out.push(bounded(joint(*p, w)));
    }
    out
}

/// The least distance between two lists of shapes; a box that is already farther than the best found skips its exact test.
pub fn region_dist(a: &[Bounded], b: &[Bounded]) -> f64 {
    let mut best = f64::INFINITY;
    for x in a {
        for y in b {
            if box_gap(x, y) >= best {
                continue;
            }
            best = best.min(shape_dist(&x.shape, &y.shape));
            if best == 0.0 {
                return 0.0;
            }
        }
    }
    best
}

/// The polyline with its first and/or last point pulled `d` px inward (to test a corridor against the rooms it opens into).
pub fn trim_ends(pts: &[V], d0: f64, d1: f64) -> Vec<V> {
    let mut out = pts.to_vec();
    let n = out.len();
    let pull = |out: &mut Vec<V>, i: usize, j: usize, d: f64| {
        let l = dist(out[i], out[j]);
        if l > d {
            out[i] = add(out[i], mul(sub(out[j], out[i]), d / l));
        }
    };
    if d0 > 0.0 {
        pull(&mut out, 0, 1, d0);
    }
    if d1 > 0.0 {
        pull(&mut out, n - 1, n - 2, d1);
    }
    out
}

pub fn polyline_length(pts: &[V]) -> f64 {
    pts.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// The direction index (0..7) of the segment a→b, or -1 when it is not in Θ₈.
pub fn seg_dir(a: V, b: V) -> i32 {
    let l = dist(a, b);
    if l == 0.0 {
        return -1;
    }
    let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
    for (k, d) in DIRS.iter().enumerate() {
        if (u[0] - d[0]).abs() < 1e-6 && (u[1] - d[1]).abs() < 1e-6 {
            return k as i32;
        }
    }
    -1
}
