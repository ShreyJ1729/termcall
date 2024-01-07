// TODO fix memory leak

use std::io::Write;

use sixel_sys::{
    sixel_allocator_new, sixel_dither_get, sixel_dither_get_palette, sixel_dither_unref,
    sixel_encoder_create, sixel_encoder_encode_bytes, sixel_encoder_unref,
    sixel_helper_scale_image, sixel_output_create, sixel_output_unref, Encoder,
};

use libc::c_uchar;
use opencv::{
    core::{self, Mat, Vector},
    imgcodecs,
    prelude::*,
    videoio,
};

use std::ffi::CStr;
use std::io::{self, Read};
use std::os::raw::c_void;
use std::ptr;
use std::slice;
use termion::{raw::IntoRawMode, screen::AlternateScreen};

unsafe fn cleanup(
    encoder: *mut Encoder,
    dither: *mut sixel_sys::Dither,
    output: *mut sixel_sys::Output,
) {
    sixel_encoder_unref(encoder);
    sixel_dither_unref(dither);
    sixel_output_unref(output);

    // clear scrollback buffer so that Iterm2 doesn't use massive amounts of memory
    print!("\x1B]1337;ClearScrollback\x07");
}

fn main() {
    unsafe {
        // Create a SIXEL output object that writes to stdout
        let output = sixel_output_create(None, ptr::null_mut() as *mut c_void);
        if output.is_null() {
            eprintln!("Failed to create sixel output");
            return;
        }

        // Create a SIXEL dither object
        let dither = sixel_dither_get(sixel_sys::BuiltinDither::XTerm256);
        if dither.is_null() {
            eprintln!("Failed to create sixel dither");
            cleanup(ptr::null_mut(), ptr::null_mut(), output);
            return;
        }

        // Create a SIXEL encoder object
        let encoder = sixel_encoder_create();
        if encoder.is_null() {
            eprintln!("Failed to create sixel encoder");
            cleanup(ptr::null_mut(), dither, output);
            return;
        }

        let mut stdout = io::stdout().into_raw_mode().unwrap();

        // minimum resolution that can be captured at
        const CAMERA_WIDTH: i32 = 640;
        const CAMERA_HEIGHT: i32 = 480;

        let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY).unwrap();
        cam.set(videoio::CAP_PROP_FRAME_WIDTH, CAMERA_WIDTH as f64)
            .unwrap();
        cam.set(videoio::CAP_PROP_FRAME_HEIGHT, CAMERA_HEIGHT as f64)
            .unwrap();

        let opened = videoio::VideoCapture::is_opened(&cam).unwrap();
        if !opened {
            eprintln!("Unable to open default camera!");
            cleanup(encoder, dither, output);
            return;
        }

        let begin = std::time::Instant::now();
        const CLEAR_BUFFER_INTERVAL: u64 = 10;
        let mut cleared_scrollback = false;

        let mut frame = Mat::default();
        let mut buf = Vector::new();
        let mut allocator = std::ptr::null_mut();
        let ppallocator = &mut allocator as *mut _;

        while begin.elapsed().as_secs() < 60 {
            let start = std::time::Instant::now();

            // clear scrollback buffer every so often
            let at_interval = begin.elapsed().as_secs() % CLEAR_BUFFER_INTERVAL == 0;

            if !at_interval {
                cleared_scrollback = false;
            }

            if at_interval && !cleared_scrollback {
                print!("\x1B]1337;ClearScrollback\x07");
                cleared_scrollback = true;
            }

            match writeln!(stdout, "{}", termion::cursor::Goto(1, 1)) {
                Err(e) => {
                    eprintln!("{}stdout error: {}", termion::screen::ToMainScreen, e);
                    cleanup(encoder, dither, output);
                    return;
                }
                Ok(_) => {}
            }

            // webcame get image
            cam.read(&mut frame).unwrap();

            imgcodecs::imencode(".bmp", &frame, &mut buf, &Vector::new()).unwrap();

            let img = image::load_from_memory(&buf.to_vec()).unwrap();

            // get width, height, and bytes
            let width = img.width();
            let height = img.height();
            let image_bytes = img.into_rgb8();

            let status = sixel_allocator_new(ppallocator, None, None, None, None);

            match status {
                sixel_sys::status::OK => {}
                _ => {
                    eprintln!("Failed to create sixel allocator");
                    let error_message = sixel_sys::sixel_helper_format_error(status);
                    let message = CStr::from_ptr(error_message);
                    eprintln!("Error: {}", message.to_str().unwrap());
                    cleanup(encoder, dither, output);
                    return;
                }
            }

            let src = image_bytes.as_ptr();
            let srcw = width as i32;
            let srch = height as i32;
            let pixelformat = sixel_sys::PixelFormat::RGB888;
            let scale_constant = 1.5;
            let dstw = (srcw as f64 * scale_constant) as i32;
            let dsth = (srch as f64 * scale_constant) as i32;
            let method_for_resampling = sixel_sys::ResamplingMethod::Nearest;

            let dst_size = (dstw * dsth * 3) as usize;
            let dst = libc::malloc(dst_size) as *mut c_uchar;
            if dst.is_null() {
                eprintln!("Failed to allocate memory for scaled image");
                cleanup(encoder, dither, output);
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
                    cleanup(encoder, dither, output);
                    return;
                }
            }

            // Encode the bytes into a SIXEL image
            let status = sixel_encoder_encode_bytes(
                encoder,
                dst,
                dstw,
                dsth,
                sixel_sys::PixelFormat::RGB888,
                sixel_dither_get_palette(dither),
                sixel_sys::sixel_dither_get_num_of_palette_colors(dither),
            );

            // Check for errors
            match status {
                sixel_sys::status::OK => {
                    let elapsed = start.elapsed();
                    let fps = 1 as f64 / elapsed.as_secs_f64();
                    println!("width: {}, height: {}, fps: {}", dstw, dsth, fps);
                }
                _ => {
                    eprintln!("Failed to encode image");
                    let error_message = sixel_sys::sixel_helper_format_error(status);
                    let message = CStr::from_ptr(error_message);
                    eprintln!("Error: {}", message.to_str().unwrap());
                    cleanup(encoder, dither, output);
                    return;
                }
            }
        }

        cleanup(encoder, dither, output);
    }
}
