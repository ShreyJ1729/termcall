mod camera;
mod microphone;
mod speaker;
mod stats;
mod terminal;

use camera::Camera;
use microphone::Microphone;
use speaker::Speaker;
use stats::get_memory_usage;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use terminal::Terminal;

const CAMERA_WIDTH: f64 = 640 as f64;
const CAMERA_HEIGHT: f64 = 480 as f64;
const CAMERA_FPS: f64 = 24 as f64;

fn main() {
    let mut camera = Camera::new(CAMERA_WIDTH, CAMERA_HEIGHT, CAMERA_FPS).unwrap();
    let mut microphone = Microphone::new();
    let mut speaker = Speaker::new();
    let mut terminal = Terminal::new();

    let mut frame_count = 0;
    let mut begin = std::time::Instant::now();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    terminal.clear();
    terminal.hide_cursor();

    while running.load(Ordering::SeqCst) {
        let (terminal_width, terminal_height, size_changed) = terminal.get_size();

        assert!(camera.read_frame());
        camera.resize_frame(terminal_width as f64, (terminal_height - 1) as f64, true);
        camera.change_color_depth(24);

        // clear terminal if size changes (to avoid artifacts)
        if size_changed {
            terminal.clear();
        }

        terminal.goto_topleft();

        terminal.write_frame(camera.get_frame_mirrored());

        let stats = format!(
            "mem usage: {:.0}MB | pixels: {} ({}x{}) | fps: {:.0}",
            get_memory_usage(),
            camera.get_frame_num_pixels(),
            camera.get_frame_width(),
            camera.get_frame_height(),
            frame_count as f64 / begin.elapsed().as_secs_f64()
        );

        terminal.write_to_bottomright(&stats);

        // calculate fps based on moving frame rate every second
        if begin.elapsed().as_secs() > 1 {
            frame_count = 0;
            begin = std::time::Instant::now();
        }
        frame_count += 1;

        std::thread::sleep(Duration::from_millis(10));
    }
    terminal.show_cursor();
}
