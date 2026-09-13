//! Wisp: a cache-friendly Drosophila-inspired brain + tiny 3D arena.
//!
//! The simulation is deliberately a reduced sub-graph rather than a claim to be
//! a biologically complete 166k-neuron fly brain. The initial target is ~5,000
//! LIF neurons, flat storage, sparse connectivity, and a very small renderer.

mod brain;
mod physics;
mod render;

use std::sync::Arc;
use std::time::{Duration, Instant};

use brain::{Brain, BrainConfig, BrainSnapshot};
use physics::{Food, Fly};
use render::Renderer;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::Window;

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().expect("failed to create event loop");
    let window = Arc::new(
        event_loop
            .create_window(Window::default_attributes().with_title("Wisp — Drosophila Brain Arena"))
            .expect("failed to create window"),
    );

    let mut renderer = pollster::block_on(Renderer::new(window.clone()));
    let mut brain = Brain::new(BrainConfig::default());
    let mut fly = Fly::default();
    let food = Food::default();

    let mut last_frame = Instant::now();
    let mut accumulator = Duration::ZERO;
    // A fixed neural timestep keeps learning behavior deterministic while the
    // renderer is allowed to run independently at the display refresh rate.
    const NEURAL_DT: Duration = Duration::from_micros(2_000);

    event_loop
        .run(move |event, target| {
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => target.exit(),
                    WindowEvent::KeyboardInput { event, .. }
                        if event.state == ElementState::Pressed
                            && event.logical_key == winit::keyboard::Key::Named(
                                winit::keyboard::NamedKey::Escape,
                            ) => target.exit(),
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let frame_dt = (now - last_frame).min(Duration::from_millis(50));
                        last_frame = now;
                        accumulator += frame_dt;

                        while accumulator >= NEURAL_DT {
                            let visual = fly.visual_input(&food);
                            let snapshot: BrainSnapshot = brain.step(visual, 0.002);
                            fly.apply_motor(snapshot.turn, snapshot.forward, 0.002);
                            fly.integrate(0.002);

                            if fly.collides_with(&food) {
                                brain.inject_dopamine(1.0);
                                fly.reset_near_origin();
                            }

                            accumulator -= NEURAL_DT;
                        }

                        match renderer.render(&fly, &food) {
                            Ok(()) => {}
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                renderer.resize(renderer.size());
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                            Err(wgpu::SurfaceError::Timeout) => {}
                        }
                    }
                    WindowEvent::Resized(size) => renderer.resize(size),
                    _ => {}
                },
                Event::AboutToWait => window.request_redraw(),
                _ => {}
            }
        })
        .expect("event loop failed");
}
