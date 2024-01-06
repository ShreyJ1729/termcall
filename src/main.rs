// TODO fix memory leak

use std::io::Write;

use sixel_sys::{
    sixel_allocator_free, sixel_allocator_malloc, sixel_allocator_new, sixel_allocator_realloc,
    sixel_dither_get, sixel_dither_get_palette, sixel_dither_unref, sixel_encoder_create,
    sixel_encoder_encode_bytes, sixel_encoder_unref, sixel_helper_scale_image, sixel_output_create,
    sixel_output_unref,
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
use termion::raw::IntoRawMode;

unsafe extern "C" fn write_function(data: *mut i8, size: i32, _priv_data: *mut c_void) -> i32 {
    let data_slice = unsafe { slice::from_raw_parts(data as *const u8, size as usize) };

    match io::stdout().write_all(data_slice) {
        Ok(_) => sixel_sys::status::OK as i32,
        Err(_) => return sixel_sys::status::RUNTIME_ERROR as i32,
    }
}

fn main() {
    unsafe {
        // Create a SIXEL output object that writes to stdout
        let output = sixel_output_create(Some(write_function), ptr::null_mut() as *mut c_void);
        if output.is_null() {
            eprintln!("Failed to create sixel output");
            return;
        }

        // Create a SIXEL dither object
        let dither = sixel_dither_get(sixel_sys::BuiltinDither::G1);
        if dither.is_null() {
            eprintln!("Failed to create sixel dither");
            sixel_output_unref(output);
            return;
        }

        // Create a SIXEL encoder object
        let encoder = sixel_encoder_create();
        if encoder.is_null() {
            eprintln!("Failed to create sixel encoder");
            sixel_dither_unref(dither);
            sixel_output_unref(output);
            return;
        }

        let mut stdout = io::stdout().into_raw_mode().unwrap();
        let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY).unwrap();
        let opened = videoio::VideoCapture::is_opened(&cam).unwrap();
        if !opened {
            panic!("Unable to open default camera!");
        }

        let begin = std::time::Instant::now();
        // clear screen
        write!(stdout, "{}", termion::clear::All).unwrap();
        while begin.elapsed().as_secs() < 10 {
            let start = std::time::Instant::now();

            match writeln!(stdout, "{}", termion::cursor::Goto(1, 1)) {
                Err(e) => {
                    eprintln!("{}stdout error: {}", termion::screen::ToMainScreen, e);
                    break;
                }
                Ok(_) => {}
            }

            // webcame get image
            let mut frame = Mat::default();
            cam.read(&mut frame).unwrap();

            let mut buf = Vector::new();
            imgcodecs::imencode(".png", &frame, &mut buf, &Vector::new()).unwrap();
            let img = image::load_from_memory(&buf.to_vec()).unwrap();

            // scale down to 720p
            let img = img.resize(640, 480, image::imageops::FilterType::Nearest);

            // get width, height, and bytes
            let width = img.width();
            let height = img.height();
            let image_bytes = img.into_rgb8();

            // scale up the image 2x
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
                }
            }

            let src = image_bytes.as_ptr();
            let srcw = width as i32;
            let srch = height as i32;
            let pixelformat = sixel_sys::PixelFormat::RGB888;
            let scale_constant = 1.25;
            let dstw = (srcw as f64 * scale_constant) as i32;
            let dsth = (srch as f64 * scale_constant) as i32;
            let method_for_resampling = sixel_sys::ResamplingMethod::Bilinear;

            let dst_size = (dstw * dsth * 3) as usize;
            let dst = libc::malloc(dst_size) as *mut c_uchar;
            if dst.is_null() {
                eprintln!("Failed to allocate memory for scaled image");
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
                }
            }

            // Encode the bytes into a SIXEL image
            let status = sixel_encoder_encode_bytes(
                encoder,
                dst,
                dstw as i32,
                dsth as i32,
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
                    return;
                }
            }
        }

        // Clean up
        sixel_encoder_unref(encoder);
        sixel_dither_unref(dither);
        sixel_output_unref(output);
    }
}
