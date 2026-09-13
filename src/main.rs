//! Wisp executable: fixed-step neural simulation plus a tiny low-poly 3D arena.

mod brain;
mod camera;
mod physics;
mod render;
mod sim;

use std::sync::Arc;
use std::time::Instant;

use render::Renderer;
use sim::{Simulation, NEURAL_DT};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

#[derive(Clone, Copy, Debug, Default)]
struct Args {
    benchmark: Option<f32>,
    train: Option<f32>,
    headless: bool,
    threads: usize,
}

fn parse_args() -> Args {
    let mut args = Args { threads: 2, ..Default::default() };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--headless" => args.headless = true,
            "--benchmark" => args.benchmark = parse_seconds(&mut it, 5.0),
            "--train" => args.train = parse_seconds(&mut it, 60.0),
            "--threads" => {
                if let Some(value) = it.next().and_then(|value| value.parse::<usize>().ok()) {
                    args.threads = value.clamp(1, 4);
                }
            }
            "--help" | "-h" => {
                println!(
                    "Wisp\n  --benchmark [seconds]  CPU benchmark\n  --train [seconds]      deterministic reward-learning run\n  --headless              run the CPU simulation without a window\n  --threads N             Rayon workers, 1..4"
                );
                std::process::exit(0);
            }
            _ => {}
        }
    }
    args
}

fn parse_seconds<I>(it: &mut I, default: f32) -> Option<f32>
where
    I: Iterator<Item = String>,
{
    Some(it.next().and_then(|value| value.parse::<f32>().ok()).unwrap_or(default).max(0.1))
}

fn configure_threads(threads: usize) {
    let _ = rayon::ThreadPoolBuilder::new().num_threads(threads).build_global();
}

fn run_benchmark(seconds: f32, threads: usize) {
    configure_threads(threads);
    let mut simulation = Simulation::default();
    let target_steps = (seconds / NEURAL_DT) as usize;
    let start = Instant::now();
    simulation.steps(target_steps);
    let elapsed = start.elapsed().as_secs_f64().max(f64::EPSILON);
    let steps_per_second = target_steps as f64 / elapsed;
    let neurons_per_second = steps_per_second * simulation.brain.neuron_count() as f64;
    let synapses_per_second = steps_per_second * simulation.brain.synapse_count() as f64;
    let average_step_ms = elapsed * 1000.0 / target_steps.max(1) as f64;
    println!(
        "Wisp benchmark\n  threads: {threads}\n  neurons: {}\n  synapses: {}\n  steps: {target_steps}\n  elapsed: {elapsed:.3} s\n  steps/sec: {steps_per_second:.0}\n  neurons/sec: {neurons_per_second:.0}\n  synapses/sec: {synapses_per_second:.0}\n  avg step: {average_step_ms:.4} ms\n  brain memory: {} KiB\n  process RSS: {} KiB\n  rewards: {}\n  checksum: {:016x}",
        simulation.brain.neuron_count(),
        simulation.brain.synapse_count(),
        simulation.brain.memory_bytes() / 1024,
        process_rss_bytes() / 1024,
        simulation.stats.rewards,
        simulation.brain.weight_checksum(),
    );
}

fn run_training(seconds: f32, threads: usize) {
    configure_threads(threads);
    let mut simulation = Simulation::default();
    let episodes = ((seconds / 1.0).round() as usize).max(1);
    let cue_steps = (0.8 / NEURAL_DT) as usize;
    println!("Wisp training | episodes={episodes} | cue=0.8s | threads={threads}");
    let mut previous_distance = f32::INFINITY;
    for episode in 1..=episodes {
        simulation.reset_episode();
        simulation.set_food_ahead(4.0);
        simulation.steps(cue_steps);
        let distance = simulation.stats.min_food_distance;
        let rewards_before = simulation.stats.rewards;

        // The reward is delivered by a real collision, after the cue exposure.
        simulation.force_food_at_fly();
        simulation.step();
        let changed = simulation.brain.last_changed_synapses();
        let improved = if previous_distance.is_finite() {
            previous_distance - distance
        } else {
            0.0
        };
        println!(
            "episode={episode} reward={} min_distance={distance:.3} improvement={improved:+.3} changed_synapses={changed}",
            simulation.stats.rewards - rewards_before
        );
        previous_distance = distance;
    }
    println!(
        "training complete | rewards={} | dopamine={:.3} | checksum={:016x} | changed_synapses={} | mean_weight={:.5}->{:.5}",
        simulation.stats.rewards,
        simulation.brain.dopamine(),
        simulation.brain.weight_checksum(),
        simulation.brain.last_changed_synapses(),
        simulation.brain.last_mean_weight_before(),
        simulation.brain.last_mean_weight_after(),
    );
}

fn process_rss_bytes() -> usize {
    #[cfg(target_os = "linux")]
    {
        if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
            if let Some(pages) = statm.split_whitespace().nth(1).and_then(|value| value.parse::<usize>().ok()) {
                return pages * 4096;
            }
        }
    }
    0
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    simulation: Simulation,
    last_frame: Instant,
    accumulator: f32,
    stats_timer: f32,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            renderer: None,
            simulation: Simulation::default(),
            last_frame: Instant::now(),
            accumulator: 0.0,
            stats_timer: 0.0,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Wisp — Drosophila Brain Arena"))
                .expect("failed to create window"),
        );
        match pollster::block_on(Renderer::new(window.clone())) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
                self.window = Some(window);
                self.last_frame = Instant::now();
            }
            Err(error) => {
                eprintln!("Wisp renderer initialization failed: {error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && event.logical_key == Key::Named(NamedKey::Escape) => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() { renderer.resize(size); }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32().min(0.05);
                self.last_frame = now;
                self.accumulator += dt;
                self.stats_timer += dt;
                let mut steps = 0;
                while self.accumulator >= NEURAL_DT && steps < 16 {
                    self.simulation.step();
                    self.accumulator -= NEURAL_DT;
                    steps += 1;
                }
                if steps == 16 && self.accumulator >= NEURAL_DT { self.accumulator = 0.0; }
                if self.stats_timer >= 2.0 {
                    self.stats_timer = 0.0;
                    log::info!(
                        "Wisp | pos=({:.2},{:.2}) vel=({:.2},{:.2}) dopamine={:.3} rewards={} checksum={:016x}",
                        self.simulation.fly.position[0],
                        self.simulation.fly.position[2],
                        self.simulation.fly.velocity[0],
                        self.simulation.fly.velocity[2],
                        self.simulation.brain.dopamine(),
                        self.simulation.stats.rewards,
                        self.simulation.brain.weight_checksum(),
                    );
                }
                if let Some(renderer) = self.renderer.as_mut() {
                    match renderer.render(&self.simulation.fly, &self.simulation.food, dt) {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => renderer.resize(renderer.size()),
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(wgpu::SurfaceError::Timeout) => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() { window.request_redraw(); }
    }
}

fn main() {
    env_logger::init();
    let args = parse_args();
    if let Some(seconds) = args.benchmark { run_benchmark(seconds, args.threads); return; }
    if let Some(seconds) = args.train { run_training(seconds, args.threads); return; }
    configure_threads(args.threads);
    if args.headless {
        let mut simulation = Simulation::default();
        simulation.steps(2_500);
        println!(
            "Wisp headless OK | steps={} rewards={} checksum={:016x}",
            simulation.stats.steps, simulation.stats.rewards, simulation.brain.weight_checksum()
        );
        return;
    }

    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("event loop failed");
}
