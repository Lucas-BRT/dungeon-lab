//! Writes a floor as a standalone SVG: `cargo run -p tapiti-render --example export_svg -- <size small|medium|large|huge> <seed> <out.svg> [dark|light] [--grid] [--labels]`.

use tapiti_core::{
    generate::generate,
    params::{params_of_size, Size},
};
use tapiti_render::{floor_svg, Style, Theme};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [size, seed, out, rest @ ..] = args.as_slice() else {
        eprintln!(
            "usage: export_svg <size small|medium|large|huge> <seed> <out.svg> [dark|light] [--grid] [--labels]"
        );
        std::process::exit(2);
    };
    let size = *Size::ALL
        .iter()
        .find(|s| s.name() == size)
        .expect("size is small, medium, large or huge");
    let seed: u32 = seed.parse().expect("seed is a number");
    let theme = if rest.iter().any(|a| a == "light") {
        Theme::Light
    } else {
        Theme::Dark
    };
    let style = Style {
        theme,
        grid: rest.iter().any(|a| a == "--grid"),
        labels: rest.iter().any(|a| a == "--labels"),
        ..Style::default()
    };
    let svg = floor_svg(&generate(seed, &params_of_size(size)), &style);
    std::fs::write(out, svg).expect("could not write the SVG");
}
