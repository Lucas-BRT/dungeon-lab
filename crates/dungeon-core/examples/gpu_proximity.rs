//! Experimental CPU/GPU proximity graph comparison.
//! Run with `cargo run -p dungeon-lab-core --example gpu_proximity --release`.

use dungeon_lab_core::{generate, params};
use std::{
    error::Error,
    sync::mpsc,
    time::{Duration, Instant},
};

const ITERATIONS: usize = 25;
const SHADER: &str = r#"
struct Config {
    count: u32,
    kind: u32,
    near_squared: f32,
    _padding: u32,
};

@group(0) @binding(0) var<storage, read> centers: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> pairs: array<u32>;
@group(0) @binding(2) var<uniform> config: Config;

fn distance_squared(a: vec2<f32>, b: vec2<f32>) -> f32 {
    let delta = a - b;
    return dot(delta, delta);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.x;
    let count = config.count;
    if (index >= count * count) { return; }

    let i = index / count;
    let j = index % count;
    if (i >= j) {
        pairs[index] = 0u;
        return;
    }

    let a = centers[i];
    let b = centers[j];
    let ab = distance_squared(a, b);
    var connected = true;

    if (config.kind == 2u) {
        connected = ab <= config.near_squared;
    } else {
        for (var k = 0u; k < count; k += 1u) {
            if (k == i || k == j) { continue; }
            let c = centers[k];
            if (config.kind == 0u) {
                connected = !(max(distance_squared(a, c), distance_squared(b, c)) < ab);
            } else {
                connected = !(distance_squared(c, (a + b) * 0.5) < ab * 0.25);
            }
            if (!connected) { break; }
        }
    }

    pairs[index] = select(0u, 1u, connected);
}
"#;

struct GpuProximity {
    device: wgpu::Device,
    queue: wgpu::Queue,
    centers: wgpu::Buffer,
    pairs: wgpu::Buffer,
    config: wgpu::Buffer,
    readback: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
}

impl GpuProximity {
    async fn new(count: usize) -> Result<(Self, wgpu::AdapterInfo), Box<dyn Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await?;
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("proximity graph experiment"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("proximity pairs"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let center_bytes = (count * 8).max(8);
        let pair_bytes = (count * count * 4).max(4);
        let centers = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("room centers"),
            size: center_bytes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pairs = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("proximity flags"),
            size: pair_bytes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let config = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("proximity settings"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("proximity readback"),
            size: pair_bytes as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("proximity bindings"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: centers.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: pairs.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: config.as_entire_binding(),
                },
            ],
        });

        Ok((
            Self {
                device,
                queue,
                centers,
                pairs,
                config,
                readback,
                bind_group,
                pipeline,
            },
            info,
        ))
    }

    fn run(
        &self,
        points: &[[f64; 2]],
        kind: params::Proximity,
        near: f64,
    ) -> (Vec<[usize; 2]>, Duration) {
        let count = points.len();
        let mut center_bytes = Vec::with_capacity(points.len() * 8);
        for point in points {
            center_bytes.extend_from_slice(&(point[0] as f32).to_ne_bytes());
            center_bytes.extend_from_slice(&(point[1] as f32).to_ne_bytes());
        }
        let kind = match kind {
            params::Proximity::Rng => 0u32,
            params::Proximity::Gabriel => 1,
            params::Proximity::Near => 2,
        };
        let mut config = Vec::with_capacity(16);
        config.extend_from_slice(&(count as u32).to_ne_bytes());
        config.extend_from_slice(&kind.to_ne_bytes());
        config.extend_from_slice(&((near * near) as f32).to_ne_bytes());
        config.extend_from_slice(&0u32.to_ne_bytes());

        let start = Instant::now();
        self.queue.write_buffer(&self.centers, 0, &center_bytes);
        self.queue.write_buffer(&self.config, 0, &config);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("proximity compute"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(((count * count) as u32).div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(
            &self.pairs,
            0,
            &self.readback,
            0,
            (count * count * 4) as u64,
        );
        self.queue.submit([encoder.finish()]);

        let slice = self.readback.slice(..);
        let (sent, received) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sent.send(result);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        received.recv().unwrap().unwrap();
        let data = slice.get_mapped_range();
        let mut output = Vec::new();
        for i in 0..count {
            for j in (i + 1)..count {
                let index = (i * count + j) * 4;
                if u32::from_ne_bytes(data[index..index + 4].try_into().unwrap()) != 0 {
                    output.push([i, j]);
                }
            }
        }
        drop(data);
        self.readback.unmap();
        (output, start.elapsed())
    }
}

fn cpu_pairs(points: &[[f64; 2]], kind: params::Proximity, near: f64) -> Vec<[usize; 2]> {
    let distance_squared = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
    let mut output = Vec::new();
    for i in 0..points.len() {
        for j in (i + 1)..points.len() {
            let (a, b) = (points[i], points[j]);
            let ab = distance_squared(a, b);
            let connected = match kind {
                params::Proximity::Near => ab <= near * near,
                params::Proximity::Rng => !(0..points.len()).any(|k| {
                    k != i
                        && k != j
                        && distance_squared(a, points[k]).max(distance_squared(b, points[k])) < ab
                }),
                params::Proximity::Gabriel => !(0..points.len()).any(|k| {
                    k != i
                        && k != j
                        && distance_squared(points[k], [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5])
                            < ab / 4.0
                }),
            };
            if connected {
                output.push([i, j]);
            }
        }
    }
    output
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut examples: Vec<(String, params::Params, Vec<[f64; 2]>)> = [1, 3, 5]
        .into_iter()
        .map(|level| {
            let p = params::params_of_level(level);
            let dungeon = generate::generate(0xD06E_0000 + level as u32, &p);
            let centers: Vec<_> = dungeon.rooms.iter().map(|room| room.center).collect();
            (format!("nível {level}"), p, centers)
        })
        .collect();

    let p30 = params::params_of_level(5);
    let spacing = [p30.cell_w + p30.gap, p30.cell_h + p30.gap];
    let mut grid30 = Vec::with_capacity(30 * 30);
    for y in 0..30 {
        for x in 0..30 {
            grid30.push([x as f64 * spacing[0], y as f64 * spacing[1]]);
        }
    }
    examples.push(("grade 30x30".into(), p30, grid30));

    let max_rooms = examples
        .iter()
        .map(|(_, _, centers)| centers.len())
        .max()
        .unwrap_or(1);
    let (gpu, info) = match pollster::block_on(GpuProximity::new(max_rooms)) {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("nenhum adaptador wgpu disponível: {error}");
            return Ok(());
        }
    };
    println!(
        "adaptador: {} ({:?}, {:?})",
        info.name, info.backend, info.device_type
    );

    for (name, p, points) in examples {
        let cpu = cpu_pairs(&points, p.proximity, p.near);
        let iterations = if points.len() > 100 { 5 } else { ITERATIONS };
        let mut gpu_times = Vec::with_capacity(iterations);
        let mut result = Vec::new();
        for _ in 0..2 {
            (result, _) = gpu.run(&points, p.proximity, p.near);
        }
        assert_eq!(cpu, result, "grafo divergente: {name}");
        let mut cpu_times = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            std::hint::black_box(cpu_pairs(&points, p.proximity, p.near));
            cpu_times.push(start.elapsed());
        }
        for _ in 0..iterations {
            let (_, elapsed) = gpu.run(&points, p.proximity, p.near);
            gpu_times.push(elapsed);
        }
        let cpu_median = median(cpu_times).as_secs_f64() * 1_000.0;
        let gpu_median = median(gpu_times).as_secs_f64() * 1_000.0;
        println!(
            "{name}: {} salas, {} pares; CPU {:.3} ms, GPU+readback {:.3} ms, {:.1}x GPU/CPU",
            points.len(),
            cpu.len(),
            cpu_median,
            gpu_median,
            gpu_median / cpu_median.max(f64::MIN_POSITIVE)
        );
    }
    Ok(())
}
