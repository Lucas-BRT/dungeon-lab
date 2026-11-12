//! §7.2 Shape classes (weights by block size, a Potts model for the homophily, then the caps) and the room fitted in each block.

use super::*;

/// Gives every live block a room. Returns the island of each room when the growth layout is on (empty otherwise).
#[allow(clippy::too_many_arguments)]
pub(super) fn build_rooms(
    g: &mut Gen<'_>,
    blocks: &[Block],
    live: &[usize],
    alive: &[bool],
    neighbors: &[Vec<usize>],
    box_of: &dyn Fn(&Block) -> Bx,
    world: Bx,
    progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
) -> Vec<usize> {
    let p = g.p;
    /* §7.2 Shape classes: weights by block size, a Potts model for the homophily, then the caps. */
    let weight_of = |c: ShapeClass, cells: i32| {
        if c == ShapeClass::Star && cells < p.star_cells {
            0.0
        } else {
            p.weights.get(c).max(0.0)
        }
    };
    let mut cls: HashMap<usize, usize> = HashMap::new();
    for &i in live {
        let cells = blocks[i].w * blocks[i].h;
        let ws: Vec<f64> = CLASSES.iter().map(|c| weight_of(*c, cells)).collect();
        let k = g.pick_class(&ws);
        cls.insert(i, k);
    }
    if p.homophily != 0.0 {
        for _sweep in 0..4 {
            for &i in live {
                let cells = blocks[i].w * blocks[i].h;
                let nb: Vec<usize> = neighbors[i].iter().copied().filter(|n| alive[*n]).collect();
                let ws: Vec<f64> = CLASSES
                    .iter()
                    .enumerate()
                    .map(|(ci, c)| {
                        weight_of(*c, cells)
                            * (p.homophily
                                * nb.iter().filter(|n| cls.get(n) == Some(&ci)).count() as f64)
                                .exp()
                    })
                    .collect();
                let k = g.pick_class(&ws);
                cls.insert(i, k);
            }
        }
    }
    let caps: Vec<usize> = CLASSES
        .iter()
        // A quota is never more than the rooms of the floor, so a quota of "as many as fit" follows the grid.
        .map(|c| (p.max_of.get(*c).max(0.0) as usize).min(live.len()))
        .collect();
    let mut counts = [0usize; 5];
    for &ci in cls.values() {
        counts[ci] += 1;
    }
    for ci in 0..CLASSES.len() {
        if counts[ci] <= caps[ci] {
            continue;
        }
        let excess: Vec<usize> = live.iter().copied().filter(|i| cls[i] == ci).collect();
        for &i in excess.iter().skip(caps[ci]) {
            if counts[ci] <= caps[ci] {
                break;
            }
            let cells = blocks[i].w * blocks[i].h;
            let mut ws: Vec<f64> = CLASSES
                .iter()
                .enumerate()
                .map(|(oi, c)| {
                    if oi != ci && counts[oi] < caps[oi] {
                        weight_of(*c, cells)
                    } else {
                        0.0
                    }
                })
                .collect();
            if ws.iter().sum::<f64>() <= 0.0 {
                // The block is too small for every class with room left (a star), so keep the cap anyway,
                // but only in classes the weights allow: a weight of 0 means never, however many rooms there are.
                for oi in 0..ws.len() {
                    if oi != ci && counts[oi] < caps[oi] && p.weights.get(CLASSES[oi]) > 0.0 {
                        ws[oi] = 1.0;
                    }
                }
            }
            let Some(new_class) = g.weighted(&ws) else {
                break;
            };
            cls.insert(i, new_class);
            counts[ci] -= 1;
            counts[new_class] += 1;
        }
    }
    if counts.iter().zip(&caps).any(|(count, cap)| count > cap) {
        g.log.push(
            "shape class caps cannot fit every room; at least one cap is exceeded".to_string(),
        );
    }

    /* Rooms: each block's shape, fitted (a rectangle when its class can't fit). */
    for &i in live.iter().filter(|_| p.layout != Layout::Growth) {
        let b = blocks[i];
        let bx = box_of(&b);
        let mut c = CLASSES[cls[&i]];
        let cells = b.w * b.h;
        let shape = match fit(c, bx, &mut g.rng, p, cells) {
            Some(s) => s,
            None => {
                g.log.push(format!(
                    "block {}: {} does not fit, rect instead",
                    i,
                    c.name()
                ));
                c = ShapeClass::Rect;
                fit(ShapeClass::Rect, bx, &mut g.rng, p, cells).expect("a rect always fits a block")
            }
        };
        let id = g.rooms.len();
        let bounds = shape_bounds(&shape);
        g.rooms.push(Room {
            id,
            cls: c,
            bounds,
            center: centroid(&shape),
            area: area_of(&shape),
            rect: Rect {
                x0: bx.x0,
                y0: bx.y0,
                x1: bx.x1,
                y1: bx.y1,
            },
            ports: ports_of(id, &shape, p),
            shape,
            used: Vec::new(),
            leaf: false,
            alive: true,
            role: Role::Plain,
        });
        report_progress(progress, || {
            GenerationEvent::RoomAdded(g.rooms[id].shape.clone())
        });
    }
    g.nodes = g.rooms.iter().map(|r| Node::Room { room: r.id }).collect();
    g.node_of_room = g.rooms.iter().map(|r| r.id).collect();

    let mut island_of: Vec<usize> = Vec::new();
    if p.layout == Layout::Growth {
        let target = if p.growth.rooms > 0 {
            p.growth.rooms as usize
        } else {
            live.len()
        };
        // The grown rooms are as large as the grid's blocks: their sizes (in cells) are the pool the growth draws from.
        let sizes: Vec<(i32, i32)> = live.iter().map(|&i| (blocks[i].w, blocks[i].h)).collect();
        island_of = grow_rooms(g, world, target, &sizes, progress);
    }
    island_of
}
