// todo when switching from main --> alternate and vv. the screen flickers
// to fix
// - right before switching, write the most recent frame to other screen
// - clear main screen buffer between switches
// mess with flags in opencv buffer writing to improve performance
use std::io::Write;

use image::ImageBuffer;
use image::Rgb;
use sixel_sys::sixel_allocator_unref;
use sixel_sys::Allocator;
use sixel_sys::{
    sixel_allocator_new, sixel_dither_get, sixel_dither_get_palette, sixel_dither_unref,
    sixel_encoder_create, sixel_encoder_encode_bytes, sixel_encoder_unref,
    sixel_helper_scale_image, sixel_output_create, sixel_output_unref, Dither, Encoder, Output,
};

use libc::c_uchar;
use opencv::{
    core::{Mat, Vector},
    imgcodecs,
    prelude::*,
    videoio,
};
use std::ffi::CStr;
use std::io;
use std::os::raw::c_void;
use std::ptr;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;

unsafe fn cleanup(
    encoder: *mut Encoder,
    dither: *mut Dither,
    output: *mut Output,
    allocator: *mut Allocator,
) {
    sixel_encoder_unref(encoder);
    sixel_dither_unref(dither);
    sixel_output_unref(output);
    sixel_allocator_unref(allocator);

    print!("\x1B]1337;ClearScrollback\x07");
    print!("{}", termion::cursor::Show);
}

unsafe fn create_sixel_objects() -> Option<(
    *mut Encoder,
    *mut Dither,
    *mut Output,
    *mut Allocator,
    *mut *mut Allocator,
)> {
    // Create a SIXEL output object that writes to stdout
    let output = sixel_output_create(None, ptr::null_mut() as *mut c_void);
    if output.is_null() {
        eprintln!("Failed to create sixel output");
        return None;
    }

    // Create a SIXEL dither object
    let dither = sixel_dither_get(sixel_sys::BuiltinDither::XTerm256);
    if dither.is_null() {
        eprintln!("Failed to create sixel dither");
        cleanup(ptr::null_mut(), ptr::null_mut(), output, ptr::null_mut());
        return None;
    }

    // Create a SIXEL encoder object
    let encoder = sixel_encoder_create();
    if encoder.is_null() {
        eprintln!("Failed to create sixel encoder");
        cleanup(ptr::null_mut(), dither, output, ptr::null_mut());
        return None;
    }

    // Create a SIXEL allocator object
    let mut allocator = std::ptr::null_mut();
    let ppallocator = &mut allocator as *mut _;
    let status = sixel_allocator_new(ppallocator, None, None, None, None);
    match status {
        sixel_sys::status::OK => {}
        _ => {
            eprintln!("Failed to create sixel allocator");
            let error_message = sixel_sys::sixel_helper_format_error(status);
            let message = CStr::from_ptr(error_message);
            eprintln!("Error: {}", message.to_str().unwrap());
            cleanup(encoder, dither, output, ptr::null_mut());
            return None;
        }
    }

    return Some((encoder, dither, output, allocator, ppallocator));
}

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

fn get_camera_image(
    cam: &mut videoio::VideoCapture,
    mut frame: &mut Mat,
    mut buf: &mut Vector<u8>,
) -> (ImageBuffer<Rgb<u8>, Vec<u8>>, u32, u32) {
    cam.read(&mut frame).unwrap();
    imgcodecs::imencode(".bmp", frame, &mut buf, &Vector::new()).unwrap();
    let img = image::load_from_memory(&buf.to_vec()).unwrap();

    // get width, height, and bytes
    let width = img.width();
    let height = img.height();
    let image_bytes = img.into_rgb8();

    return (image_bytes, width, height);
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

unsafe fn sixel_encode_bytes(
    encoder: *mut Encoder,
    dither: *mut Dither,
    bytes: *mut c_uchar,
    width: i32,
    height: i32,
    pixelformat: sixel_sys::PixelFormat,
    cam_width: f64,
    cam_height: f64,
    begin: std::time::Instant,
    output: *mut Output,
    allocator: *mut Allocator,
    frame_count: u64,
) {
    let status = sixel_encoder_encode_bytes(
        encoder,
        bytes,
        width,
        height,
        pixelformat,
        sixel_dither_get_palette(dither),
        sixel_sys::sixel_dither_get_num_of_palette_colors(dither),
    );

    match status {
        sixel_sys::status::OK => {
            let fps = frame_count as f64 / begin.elapsed().as_secs_f64();
            println!(
                "camera: {}x{} | frame: {}x{} | fps: {:.1}{}",
                cam_width,
                cam_height,
                width,
                height,
                fps,
                termion::cursor::Left(100)
            );

            println!(
                "memory usage: {:.2}MB{}",
                get_memory_usage(),
                termion::cursor::Left(100)
            );
        }
        _ => {
            eprintln!("Failed to encode image");
            let error_message = sixel_sys::sixel_helper_format_error(status);
            let message = CStr::from_ptr(error_message);
            eprintln!("Error: {}", message.to_str().unwrap());
            cleanup(encoder, dither, output, allocator);
            return;
        }
    }
}

fn main() {
    unsafe {
        let (encoder, dither, output, allocator, _) = match create_sixel_objects() {
            Some(sixel_object) => sixel_object,
            None => return,
        };

        let mut stdout = io::stdout().into_raw_mode().unwrap();
        let mut using_alt_screen = false;

        // minimum resolution that can be captured at
        const CAMERA_WIDTH: f64 = 640 as f64;
        const CAMERA_HEIGHT: f64 = 480 as f64;
        const SCALE_CONSTANT: f64 = 1.0;

        let (mut cam, cam_width, cam_height) = match setup_camera(CAMERA_WIDTH, CAMERA_HEIGHT) {
            Some(cam) => cam,
            None => return,
        };

        let begin = std::time::Instant::now();
        let mut frame_count = 0;
        const CLEAR_BUFFER_INTERVAL: u64 = 5;
        let mut cleared_scrollback = false;

        let mut frame = Mat::default();
        let mut buf = Vector::new();

        print!("{}", termion::cursor::Hide);

        while begin.elapsed().as_secs() < 20 {
            // clear scrollback buffer every so often
            let at_interval = begin.elapsed().as_secs() % CLEAR_BUFFER_INTERVAL == 0;

            if !at_interval {
                cleared_scrollback = false;
            }

            if at_interval && !cleared_scrollback {
                // if using alt screen, switch to main (auto-clears alt scrollback)
                if using_alt_screen {
                    print!("{}", termion::screen::ToMainScreen);
                } else {
                    // if using main, switch to alt and clear main buffer
                    print!("{}", termion::screen::ToAlternateScreen);
                    write!(stdout, "\x1B]1337;ClearScrollback\x07").unwrap();
                    stdout.flush().unwrap();
                }

                using_alt_screen = !using_alt_screen;
                cleared_scrollback = true;
            }

            let (image_bytes, width, height) = get_camera_image(&mut cam, &mut frame, &mut buf);

            let src = image_bytes.as_ptr();
            let srcw = width as i32;
            let srch = height as i32;
            let pixelformat = sixel_sys::PixelFormat::RGB888;
            let dstw = (srcw as f64 * SCALE_CONSTANT) as i32;
            let dsth = (srch as f64 * SCALE_CONSTANT) as i32;
            let method_for_resampling = sixel_sys::ResamplingMethod::Nearest;

            let dst_size = (dstw * dsth * 3) as usize;
            let dst = libc::malloc(dst_size) as *mut c_uchar;
            if dst.is_null() {
                eprintln!("Failed to allocate memory for scaled image");
                cleanup(encoder, dither, output, allocator);
                return;
            };

            let status = sixel_helper_scale_image(
                dst,
                src,
                srcw,
                srch,
                pixelformat,
                dstw,
                dsth,
                method_for_resampling,
                allocator,
            );
            match status {
                sixel_sys::status::OK => {}
                _ => {
                    eprintln!("Failed to scale image");
                    let error_message = sixel_sys::sixel_helper_format_error(status);
                    let message = CStr::from_ptr(error_message);
                    eprintln!("Error: {}", message.to_str().unwrap());
                    cleanup(encoder, dither, output, allocator);
                    return;
                }
            }

            goto_terminal_topleft(&mut stdout).unwrap();

            sixel_encode_bytes(
                encoder,
                dither,
                dst,
                dstw,
                dsth,
                sixel_sys::PixelFormat::RGB888,
                cam_width,
                cam_height,
                begin,
                output,
                allocator,
                frame_count,
            );
            frame_count += 1;

            // Free the memory allocated for the encoder input
            libc::free(dst as *mut c_void);
        }

        cleanup(encoder, dither, output, allocator);
    }
}
