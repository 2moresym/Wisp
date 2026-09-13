//! Wisp executable: fixed-step neural simulation plus an intentionally tiny 3D arena.

mod brain;
mod camera;
mod physics;
mod render;
mod sim;

use std::sync::Arc;
use std::time::Instant;

use render::Renderer;
use sim::{Simulation, NEURAL_DT};
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::Window;

#[derive(Clone, Copy, Debug)]
struct Args { benchmark: Option<f32>, headless: bool, threads: usize }

fn parse_args() -> Args {
    let mut benchmark=None;
    let mut headless=false;
    let mut threads=4usize;
    let mut it=std::env::args().skip(1);
    while let Some(arg)=it.next() {
        match arg.as_str() {
            "--headless" => headless=true,
            "--benchmark" => benchmark=Some(it.next().and_then(|s|s.parse::<f32>().ok()).unwrap_or(5.0).max(0.1)),
            "--threads" => if let Some(v)=it.next().and_then(|s|s.parse::<usize>().ok()){threads=v.clamp(1,4);},
            "--help"|"-h" => { println!("Wisp\n  --benchmark [seconds]  headless simulation benchmark\n  --headless              run simulation without opening a window\n  --threads N              Rayon workers, 1..4"); std::process::exit(0); }
            _ => {}
        }
    }
    Args{benchmark,headless,threads}
}

fn run_benchmark(seconds:f32,threads:usize){
    rayon::ThreadPoolBuilder::new().num_threads(threads).build_global().ok();
    let mut sim=Simulation::default();
    let target=(seconds/NEURAL_DT) as usize;
    let start=Instant::now();
    sim.steps(target);
    let elapsed=start.elapsed().as_secs_f64();
    let rate=target as f64/elapsed.max(f64::EPSILON);
    println!("Wisp benchmark | threads={threads} | steps={target} | {:.0} steps/s | {:.3} ms/step | neurons={} | synapses={} | brain_mem={} KiB | rewards={} | checksum={:016x}",rate,elapsed*1000.0/target.max(1) as f64,sim.brain.neuron_count(),sim.brain.synapse_count(),sim.brain.memory_bytes()/1024,sim.stats.rewards,sim.brain.weight_checksum());
}

fn main(){
    env_logger::init();
    let args=parse_args();
    rayon::ThreadPoolBuilder::new().num_threads(args.threads).build_global().ok();
    if let Some(seconds)=args.benchmark { run_benchmark(seconds,args.threads); return; }
    if args.headless { let mut sim=Simulation::default(); sim.steps(2_500); println!("Wisp headless OK | steps={} rewards={} checksum={:016x}",sim.stats.steps,sim.stats.rewards,sim.brain.weight_checksum()); return; }

    let event_loop=EventLoop::new().expect("failed to create event loop");
    let window=Arc::new(event_loop.create_window(Window::default_attributes().with_title("Wisp — Drosophila Brain Arena")).expect("failed to create window"));
    let mut renderer=match pollster::block_on(Renderer::new(window.clone())){Ok(r)=>r,Err(e)=>{eprintln!("Wisp renderer initialization failed: {e}");return;}};
    let mut simulation=Simulation::default();
    let mut last=Instant::now(); let mut accumulator=0.0f32; let mut stats_timer=0.0f32;
    const MAX_FRAME_DT:f32=0.05; const MAX_STEPS:usize=16;

    event_loop.run(move |event,target|match event{
        Event::WindowEvent{event,..}=>match event{
            WindowEvent::CloseRequested=>target.exit(),
            WindowEvent::KeyboardInput{event,..} if event.state==ElementState::Pressed && event.logical_key==winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)=>target.exit(),
            WindowEvent::Resized(size)=>renderer.resize(size),
            WindowEvent::RedrawRequested=>{
                let now=Instant::now();let dt=(now-last).as_secs_f32().min(MAX_FRAME_DT);last=now;accumulator+=dt;stats_timer+=dt;
                let mut steps=0;while accumulator>=NEURAL_DT&&steps<MAX_STEPS{simulation.step();accumulator-=NEURAL_DT;steps+=1;}
                if steps==MAX_STEPS&&accumulator>=NEURAL_DT{accumulator=0.0;}
                if stats_timer>=2.0{stats_timer=0.0;log::info!("Wisp | pos=({:.2},{:.2}) vel=({:.2},{:.2}) dopamine={:.3} rewards={}",simulation.fly.position[0],simulation.fly.position[2],simulation.fly.velocity[0],simulation.fly.velocity[2],simulation.brain.dopamine(),simulation.stats.rewards);}
                match renderer.render(&simulation.fly,&simulation.food){Ok(())=>{},Err(wgpu::SurfaceError::Lost|wgpu::SurfaceError::Outdated)=>renderer.resize(renderer.size()),Err(wgpu::SurfaceError::OutOfMemory)=>target.exit(),Err(wgpu::SurfaceError::Timeout)=>{}}
            }
            _=>{}
        },
        Event::AboutToWait=>window.request_redraw(),_=>{}
    }).expect("event loop failed");
}
