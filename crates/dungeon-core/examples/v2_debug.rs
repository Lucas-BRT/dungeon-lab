use dungeon_lab_core::generate::{Edge, Node};
use dungeon_lab_core::{generate, geom, params, validate};
fn main() {
    let mut shown = 0;
    let mut total = 0;
    for rooms in [30, 40] {
        let mut p = params::params_of_level(5);
        p.cols = 10; p.rows = 8; p.growth.enabled = true; p.growth.rooms = rooms;
        for s in 0..300u32 {
            let d = generate::generate(0xC000 + s, &p);
            let tree = d.rooms.iter().filter(|r| r.alive).count() - 1;
            let desc = |e: &Edge| {
                let n = |i: usize| match &d.nodes[i] { Node::Junction { p } => format!("J({:.0},{:.0})", p[0], p[1]), Node::Room { room } => format!("R{room}") };
                format!("e{} {} {}→{} pts {:?}", e.id, if e.id < tree { "tree" } else { "late" }, n(e.u), n(e.v), e.pts.iter().map(|q| (q[0].round(), q[1].round())).collect::<Vec<_>>())
            };
            for v in validate::validate(&d).into_iter().filter(|v| v.starts_with("V2")) {
                total += 1;
                if shown >= 10 { continue; }
                shown += 1;
                println!("R={rooms} seed {s}: {v}");
                let ids: Vec<usize> = v.split_whitespace().filter_map(|w| w.parse().ok()).collect();
                if v.starts_with("V2 corridors") {
                    let (a, b) = (&d.edges[ids[0]], &d.edges[ids[1]]);
                    println!("    {}\n    {}\n    dist {:.1}", desc(a), desc(b), geom::region_dist(&a.region, &b.region));
                } else if v.starts_with("V2 corridor ") {
                    let e = &d.edges[ids[0]];
                    let r = &d.rooms[ids[1]];
                    println!("    {}\n    room R{} center ({:.0},{:.0}) class {:?} doors used {:?}", desc(e), r.id, r.center[0], r.center[1], r.cls, r.used.iter().map(|q| (q.p[0].round(), q.p[1].round(), q.k)).collect::<Vec<_>>());
                    println!("    ends {:?}", e.ends.iter().map(|q| q.map(|q| (q.room, q.p[0].round(), q.p[1].round(), q.k))).collect::<Vec<_>>());
                }
            }
        }
    }
    println!("total V2 messages: {total}");
}
