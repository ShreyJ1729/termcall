mod camera;
mod microphone;
mod speaker;
mod stats;
mod terminal;

use camera::Camera;
use crossterm::{
    event,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use microphone::Microphone;
use speaker::Speaker;
use stats::get_memory_usage;
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

    // Home menu
    terminal.clear();
    terminal.write("Enter your name: ");

    // get input
    let mut name = String::new();
    loop {
        if event::poll(std::time::Duration::from_millis(50)).unwrap() {
            if let event::Event::Key(event) = event::read().unwrap() {
                if event.code == event::KeyCode::Enter {
                    break;
                } else if event.code == event::KeyCode::Backspace {
                    if name.len() > 0 {
                        name.pop();
                    }
                } else if let event::KeyCode::Char(c) = event.code {
                    name.push(c);
                }
            }
        }

        terminal.write(&name);
        terminal.flush();
    }

    // add person to list of active users on firebase here
    // todo

    // Pick who to call loop
    terminal.clear();

    // these will be pulled from active users from the firebase (exclusing self)
    let contacts = ["Alice", "Bob", "Charlie"];

    terminal.write(&format!(
        "Hello, {}! Who would you like to call?\nNames are case sensitive\n",
        name
    ));
    for (i, contact) in contacts.iter().enumerate() {
        terminal.write(&format!("{}: {}\n", i, contact));
    }
    terminal.flush();

    let mut person_to_call = String::new();
    loop {
        if event::poll(std::time::Duration::from_millis(50)).unwrap() {
            if let event::Event::Key(event) = event::read().unwrap() {
                if event.code == event::KeyCode::Enter {
                    if contacts.contains(&person_to_call.as_str()) {
                        break;
                    }
                    person_to_call.clear();
                } else if event.code == event::KeyCode::Backspace {
                    if person_to_call.len() > 0 {
                        person_to_call.pop();
                    }
                } else if let event::KeyCode::Char(c) = event.code {
                    person_to_call.push(c);
                }
            }
        }

        terminal.write(&person_to_call);
    }

    // In call loop
    terminal.clear();
    terminal.hide_cursor();
    enable_raw_mode().unwrap();

    loop {
        // If q pressed, quit
        if event::poll(std::time::Duration::from_millis(1)).unwrap() {
            if let event::Event::Key(event) = event::read().unwrap() {
                if event.code == event::KeyCode::Char('q') {
                    break;
                }
            }
        }

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
    }
    terminal.show_cursor();
    disable_raw_mode().unwrap();
}
