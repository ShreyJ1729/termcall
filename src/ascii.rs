// old ascii code

use image::DynamicImage;
use image::{GenericImageView, ImageBuffer, Luma};
use opencv::core::{Point3_, Point_};
use opencv::videoio::CAP_PROP_FPS;
use opencv::{
    core::{Mat, Vector},
    imgcodecs,
    prelude::*,
    videoio,
};
use std::io;
use std::io::Write;
use termion;

const ascii_chars: [&str; 70] = [
    " ", ".", "'", "`", "^", "\"", ",", ":", ";", "I", "l", "!", "i", ">", "<", "~", "+", "_", "-",
    "?", "]", "[", "}", "{", "1", ")", "(", "|", "\\", "/", "t", "f", "j", "r", "x", "n", "u", "v",
    "c", "z", "X", "Y", "U", "J", "C", "L", "Q", "0", "O", "Z", "m", "w", "q", "p", "d", "b", "k",
    "h", "a", "o", "*", "#", "M", "W", "&", "8", "%", "B", "@", "$",
];

const grayscale: [&str; 10] = [" ", ".", ":", "-", "=", "+", "*", "#", "%", "@"];

const SCALE_DOWN: usize = 2;

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

fn get_camera_image(cam: &mut videoio::VideoCapture, mut frame: &mut Mat) -> String {
    cam.read(&mut frame).unwrap();

    // print mat dimensions
    let rows = frame.rows() as usize;
    let cols = frame.cols() as usize;

    let data = frame.data_typed::<Point3_<u8>>().unwrap();

    let mut ascii = String::new();

    // TODO: use kernel to convert to grayscale and also scale down in process

    for (i, pixel) in data.iter().enumerate() {
        if i % SCALE_DOWN != 0 {
            continue;
        }

        let (r, c) = (i / (cols), i % (cols));

        if c == 0 {
            ascii.push_str("\n");
        }

        let (b, g, r) = (pixel.x, pixel.y, pixel.z);
        let gray = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) as u8;
        let ascii_char = ascii_chars[(gray as usize) / 6];
        ascii.push_str(ascii_char);
    }

    // only keep every 3th row since ascii chars are taller than they are wide
    let mut ascii_lines = ascii.lines();
    let mut ascii = String::new();
    while let Some(line) = ascii_lines.next() {
        ascii.push_str(line);
        ascii.push_str("\n");
        ascii_lines.next();
        ascii_lines.next();
        ascii_lines.next();
    }

    return ascii;
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
    // minimum resolution that can be captured at
    const CAMERA_WIDTH: f64 = 640 as f64;
    const CAMERA_HEIGHT: f64 = 480 as f64;

    let (mut cam, cam_width, cam_height) = match setup_camera(CAMERA_WIDTH, CAMERA_HEIGHT) {
        Some(cam) => cam,
        None => return,
    };

    let mut frame = Mat::default();
    let start = std::time::Instant::now();
    let mut framecount = 0;

    loop {
        let ascii_str = get_camera_image(&mut cam, &mut frame);

        print!("{}", ascii_str);
        println!(
            "camera: {}x{} | fps: {:.0} | frame: {}x{}, | mem: {} MB{}",
            cam_width,
            cam_height,
            framecount as f64 / start.elapsed().as_secs_f64(),
            cam_width / SCALE_DOWN as f64,
            cam_height / SCALE_DOWN as f64,
            get_memory_usage(),
            termion::cursor::Goto(1, 1)
        );

        framecount += 1;
    }
}
