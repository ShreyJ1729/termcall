use opencv::{
    core::{Mat, Point3_, Size, ToInputArray},
    imgproc,
    prelude::*,
    videoio,
};
use std::io::{self, Write};
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;

const FULLBLOCK: &str = "\u{2588}";

fn setup_camera(cam_width: f64, cam_height: f64) -> Option<(videoio::VideoCapture, f64, f64)> {
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY).unwrap();
    cam.set(videoio::CAP_PROP_FRAME_WIDTH, cam_width).unwrap();
    cam.set(videoio::CAP_PROP_FRAME_HEIGHT, cam_height).unwrap();
    cam.set(videoio::CAP_PROP_FPS, 30.0).unwrap();

    let opened = videoio::VideoCapture::is_opened(&cam).unwrap();
    if !opened {
        eprintln!("Unable to open default camera!");
        return None;
    }

    let real_camwidth = cam.get(videoio::CAP_PROP_FRAME_WIDTH).unwrap();
    let real_camheight = cam.get(videoio::CAP_PROP_FRAME_HEIGHT).unwrap();

    return Some((cam, real_camwidth, real_camheight));
}

fn get_camera_image(cam: &mut videoio::VideoCapture, mut frame: &mut Mat) -> (String, i32, i32) {
    cam.read(&mut frame).unwrap();
    // double width since ascii chars are ~2.3x as tall as they are wide
    let (orig_width, orig_height) = (2.5 * frame.cols() as f64, frame.rows());
    let orig_ratio = orig_width as f64 / orig_height as f64;

    // resample mat to terminal frame resolution
    let (term_width, term_height) = termion::terminal_size().unwrap();
    let (term_width, term_height) = (term_width as i32, term_height as i32 - 6);
    let term_ratio = term_width as f64 / term_height as f64;

    let new_size = match term_ratio > orig_ratio {
        // if term ratio bigger, term wider so resize to term height
        true => Size {
            width: (term_height as f64 * orig_ratio) as i32,
            height: term_height,
        },
        // if term ratio smaller, term taller so resize to term width
        false => Size {
            width: term_width,
            height: (term_width as f64 / orig_ratio) as i32,
        },
    };

    let mut resized_frame = Mat::default();

    imgproc::resize(
        &frame.input_array().unwrap(),
        &mut resized_frame,
        new_size,
        0.0,
        0.0,
        opencv::imgproc::INTER_LINEAR,
    )
    .unwrap();

    let width = resized_frame.cols();
    let height = resized_frame.rows();

    let data = resized_frame.data_typed::<Point3_<u8>>().unwrap();

    let mut ascii = String::new();
    let mut prev_color: String = String::from("");

    for (i, pixel) in data.iter().enumerate() {
        if i % width as usize == 0 {
            ascii.push_str("\n\r");
        }

        let (b, g, r) = (pixel.x, pixel.y, pixel.z);

        // round each r, g, b to nearest 16 for 4-bit per channel color (to minimize number of chars printed per frame)
        let r = (((r as f64 / 16.0).round() * 16.0) as u8).clamp(0, 255);
        let g = (((g as f64 / 16.0).round() * 16.0) as u8).clamp(0, 255);
        let b = (((b as f64 / 16.0).round() * 16.0) as u8).clamp(0, 255);

        let color = termion::color::Rgb(r, g, b).fg_string();
        if color == prev_color {
            ascii.push_str(FULLBLOCK);
        } else {
            ascii.push_str(&color);
            ascii.push_str(FULLBLOCK);
            prev_color = color;
        }
    }

    ascii.push_str(termion::color::Reset.fg_str());

    return (ascii, width, height);
}

fn goto_terminal_topleft(stdout: &mut RawTerminal<io::Stdout>) -> Result<(), std::io::Error> {
    write!(stdout, "{}", termion::cursor::Goto(1, 1))
}

fn get_memory_usage() -> f64 {
    // use ps -o rss= -p <pid> to get memory usage. return in MB
    let pid = std::process::id();
    let mem_usage = std::process::Command::new("ps")
        .arg("-o rss=")
        .arg("-p")
        .arg(pid.to_string())
        .output()
        .expect("failed to execute process");
    let mem_usage = String::from_utf8(mem_usage.stdout).unwrap();
    let mem_usage = mem_usage.trim().parse::<f64>().unwrap() / 1000.0;
    return mem_usage;
}
fn main() {
    let mut stdout = io::stdout().into_raw_mode().unwrap();

    // minimum resolution that can be captured at
    const CAMERA_WIDTH: f64 = 640 as f64;
    const CAMERA_HEIGHT: f64 = 480 as f64;

    let (mut cam, cam_width, cam_height) = match setup_camera(CAMERA_WIDTH, CAMERA_HEIGHT) {
        Some(cam) => cam,
        None => return,
    };

    let mut frame = Mat::default();

    print!("{}", termion::cursor::Hide);

    let mut frame_count = 0;
    let mut begin = std::time::Instant::now();

    let mut frame_width = 0;
    let mut frame_height = 0;

    loop {
        // grab new output
        let (output, new_frame_width, new_frame_height) = get_camera_image(&mut cam, &mut frame);

        // clear terminal if frame size changes (to avoid artifacts)
        if new_frame_width != frame_width || new_frame_height != frame_height {
            frame_width = new_frame_width;
            frame_height = new_frame_height;
            write!(stdout, "{}", termion::clear::All).unwrap();
        }

        // move cursor to top left
        goto_terminal_topleft(&mut stdout).unwrap();

        // write frame
        let start = std::time::Instant::now();
        write!(stdout, "{}", output).unwrap();
        write!(
            stdout,
            " printing buffer of length {:} took {:.0} ms",
            output.len(),
            start.elapsed().as_secs_f64() * 1000.0
        )
        .unwrap();

        frame_count += 1;

        // write stats
        let stats = format!(
            "camera resolution: {}x{}\n\rmemory usage: {:.0} MB\n\rframe resolution: {}x{} ({} pixels) \n\rfps: {:.0}",
            cam_width,
            cam_height,
            get_memory_usage(),
            frame_width,
            frame_height,
            frame_width * frame_height,
            frame_count as f64 / begin.elapsed().as_secs_f64()
        );
        write!(stdout, "{}", stats).unwrap();

        // calculate fps based on moving frame rate
        if begin.elapsed().as_secs() > 1 {
            frame_count = 0;
            begin = std::time::Instant::now();
        }
    }
}
