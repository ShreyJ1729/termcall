mod camera;
mod microphone;
mod stats;
mod terminal;

use camera::Camera;
use stats::get_memory_usage;
use terminal::Terminal;

const CAMERA_WIDTH: f64 = 640 as f64;
const CAMERA_HEIGHT: f64 = 480 as f64;
const CAMERA_FPS: f64 = 30 as f64;
fn main() {
    let mut camera = Camera::new(CAMERA_WIDTH, CAMERA_HEIGHT, CAMERA_FPS).unwrap();
    let mut terminal = Terminal::new();

    let mut frame_count = 0;
    let mut begin = std::time::Instant::now();

    terminal.clear();
    terminal.hide_cursor();

    loop {
        let (terminal_width, terminal_height, size_changed) = terminal.get_size();

        assert!(camera.read_frame());
        camera.resize_frame(terminal_width as f64, terminal_height as f64, true);
        camera.change_color_depth(24);

        // clear terminal if size changes (to avoid artifacts)
        if size_changed {
            terminal.clear();
        }

        terminal.goto_topleft();

        terminal.write_frame(camera.get_frame());

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
    }

    terminal.show_cursor();
}
