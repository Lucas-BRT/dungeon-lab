use tapiti_core::{generate, params, validate};
fn main() {
    for (label, growth, cols, rows, rooms, level) in [("grid 10x8 L5", false, 10, 8, 0, 5), ("grid 16x12 L5", false, 16, 12, 0, 5), ("grid L1-L4 default", false, 0, 0, 0, 0), ("grow 10x8 R=20", true, 10, 8, 20, 5), ("grow 10x8 R=30", true, 10, 8, 30, 5), ("grow 10x8 R=40", true, 10, 8, 40, 5), ("grow 16x12 R=80", true, 16, 12, 80, 5)] {
        let (mut v2, mut other, mut bad, mut n) = (0, 0, 0, 0);
        let levels: Vec<usize> = if level == 0 { vec![1, 2, 3, 4] } else { vec![level] };
        for lv in levels {
            let mut p = params::params_of_level(lv);
            if cols > 0 { p.cols = cols; p.rows = rows; }
            p.growth.enabled = growth; p.growth.rooms = rooms;
            for s in 0..200u32 {
                let v = validate::validate(&generate::generate(0xD000 + s, &p));
                n += 1;
                bad += !v.is_empty() as usize;
                v2 += v.iter().filter(|m| m.starts_with("V2")).count();
                other += v.iter().filter(|m| !m.starts_with("V2")).count();
            }
        }
        println!("{label:<20}: {bad}/{n} invalid, V2 messages {v2}, other {other}");
    }
}
