use winit::event_loop::ControlFlow::Poll;
use winit::event_loop::EventLoop;

mod app;
mod gpu;
mod params;

use app::App;

pub fn run() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
