//! Wisp entry point: fixed-step neural simulation + lightweight native renderer.

mod brain;
mod camera;
mod physics;
mod render;
mod sim;

use std::sync::Arc;
use std::time::{Duration, Instant};

use render::Renderer;
use sim::{Simulation, NEURAL_DT};
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::Window;

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().expect("failed to create event loop");
    let window = Arc::new(
        event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Wisp — Drosophila Brain Arena"),
            )
            .expect("failed to create window"),
    );

    let mut renderer = match pollster::block_on(Renderer::new(window.clone())) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("Wisp renderer initialization failed: {error}");
            return;
        }
    };

    let mut simulation = Simulation::default();
    let mut last_frame = Instant::now();
    let mut accumulator = 0.0f32;
    let mut stats_timer = 0.0f32;

    // Never allow a stalled window to execute an unbounded simulation catch-up.
    const MAX_FRAME_DT: f32 = 0.05;
    const MAX_STEPS_PER_FRAME: usize = 16;
    const STATS_INTERVAL: f32 = 2.0;

    event_loop
        .run(move |event, target| match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => target.exit(),
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed
                        && event.logical_key
                            == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) =>
                {
                    target.exit();
                }
                WindowEvent::Resized(size) => renderer.resize(size),
                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    let frame_dt = (now - last_frame)
                        .as_secs_f32()
                        .min(MAX_FRAME_DT);
                    last_frame = now;
                    accumulator += frame_dt;
                    stats_timer += frame_dt;

                    let mut steps = 0usize;
                    while accumulator >= NEURAL_DT && steps < MAX_STEPS_PER_FRAME {
                        simulation.step();
                        accumulator -= NEURAL_DT;
                        steps += 1;
                    }

                    // Drop excessive backlog instead of turning a rendering hiccup
                    // into a permanent simulation stall.
                    if steps == MAX_STEPS_PER_FRAME && accumulator >= NEURAL_DT {
                        accumulator = 0.0;
                    }

                    if stats_timer >= STATS_INTERVAL {
                        stats_timer = 0.0;
                        log::info!(
                            "Wisp | fly=({:.2},{:.2}) food=({:.2},{:.2}) dopamine={:.2}",
                            simulation.fly.position[0],
                            simulation.fly.position[2],
                            simulation.food.position[0],
                            simulation.food.position[2],
                            simulation.brain.dopamine(),
                        );
                    }

                    match renderer.render(&simulation.fly, &simulation.food) {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            renderer.resize(renderer.size());
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                        Err(wgpu::SurfaceError::Timeout) => {}
                    }
                }
                _ => {}
            },
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .expect("event loop failed");

    // Keep Duration in this module's import set stable for older compiler
    // diagnostics while the runtime uses f32 fixed-step timing above.
    let _ = Duration::ZERO;
}
