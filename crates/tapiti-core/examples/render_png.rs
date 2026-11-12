//! Draws grid and grown floors side by side as PNG files, no dependencies.
//! `cargo run --release -p tapiti-core --example render_png -- <out_dir>`

use tapiti_core::{
    generate,
    geom::{inside_poly, Shape},
    params,
};

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn png(path: &str, w: usize, h: usize, rgb: &[u8]) {
    let mut raw = Vec::with_capacity((w * 3 + 1) * h);
    for y in 0..h {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * w * 3..(y + 1) * w * 3]);
    }
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(chunks.peek().is_none() as u8);
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10];
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    std::fs::write(path, out).unwrap();
}

fn render(d: &generate::Floor, scale: f64) -> (usize, usize, Vec<u8>) {
    let (w, h) = (
        (d.grid_x.last().unwrap() * scale) as usize + 1,
        (d.grid_y.last().unwrap() * scale) as usize + 1,
    );
    let mut img = vec![250u8; w * h * 3];
    let mut put = |x: i64, y: i64, c: [u8; 3]| {
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            let i = (y as usize * w + x as usize) * 3;
            img[i..i + 3].copy_from_slice(&c);
        }
    };
    // Corridors: thick segments.
    for e in &d.edges {
        for s in e.pts.windows(2) {
            let (a, b) = (s[0], s[1]);
            let n = ((b[0] - a[0]).hypot(b[1] - a[1]) * scale).ceil().max(1.0) as i64;
            for i in 0..=n {
                let t = i as f64 / n as f64;
                let (cx, cy) = (
                    (a[0] + (b[0] - a[0]) * t) * scale,
                    (a[1] + (b[1] - a[1]) * t) * scale,
                );
                let r = (d.params.width / 2.0 * scale) as i64;
                for dy in -r..=r {
                    for dx in -r..=r {
                        put(cx as i64 + dx, cy as i64 + dy, [120, 120, 130]);
                    }
                }
            }
        }
    }
    for r in d.rooms.iter().filter(|r| r.alive) {
        let col = if r.id == d.start {
            [90, 190, 110]
        } else if r.id == d.stairs {
            [220, 90, 90]
        } else {
            [70, 90, 150]
        };
        let (x0, y0, x1, y1) = (
            (r.bounds.lo[0] * scale) as i64,
            (r.bounds.lo[1] * scale) as i64,
            (r.bounds.hi[0] * scale) as i64 + 1,
            (r.bounds.hi[1] * scale) as i64 + 1,
        );
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = [x as f64 / scale, y as f64 / scale];
                let inside = match &r.shape {
                    Shape::Poly { pts } => inside_poly(pts, p),
                    Shape::Disc { c, r } => (p[0] - c[0]).hypot(p[1] - c[1]) <= *r,
                };
                if inside {
                    put(x, y, col);
                }
            }
        }
    }
    (w, h, img)
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut base = params::params_of_level(5);
    base.cols = 16;
    base.rows = 12;
    let variants: Vec<(&str, params::Params)> = vec![
        ("grid", base.clone()),
        ("grow30_islands1", {
            let mut p = base.clone();
            p.layout = params::Layout::Growth;
            p.growth.rooms = 30;
            p
        }),
        ("grow30_islands3", {
            let mut p = base.clone();
            p.layout = params::Layout::Growth;
            p.growth.rooms = 30;
            p.growth.islands = 3;
            p
        }),
        ("grow60_islands4_c1", {
            let mut p = base.clone();
            p.layout = params::Layout::Growth;
            p.growth.rooms = 60;
            p.growth.islands = 4;
            p.growth.cluster = 1.0;
            p
        }),
    ];
    for (name, p) in variants {
        for seed in [0xE001u32, 0xE002] {
            let d = generate::generate(seed, &p);
            let (w, h, img) = render(&d, 0.18);
            png(&format!("{dir}/{name}_{:x}.png", seed), w, h, &img);
        }
    }
}
