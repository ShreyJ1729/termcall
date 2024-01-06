extern crate sixel_sys;

mod dump;
use dump::split_video_file;
use image::DynamicImage;
use sixel_sys::{
    sixel_allocator_new, sixel_dither_get, sixel_dither_get_palette, sixel_dither_unref,
    sixel_encoder_create, sixel_encoder_encode, sixel_encoder_encode_bytes, sixel_encoder_unref,
    sixel_helper_scale_image, sixel_output_create, sixel_output_unref, Allocator,
};
use std::ffi::{c_int, CStr};
use std::io::{self, Write};
use std::os::raw::c_void;
use std::ptr;
use std::slice;
use std::str::Bytes;
use termion::raw::IntoRawMode;

use std::path::Path;

unsafe extern "C" fn write_function(data: *mut i8, size: i32, _priv_data: *mut c_void) -> i32 {
    let data_slice = unsafe { slice::from_raw_parts(data as *const u8, size as usize) };

    match io::stdout().write_all(data_slice) {
        Ok(_) => sixel_sys::status::OK as i32,
        Err(_) => return sixel_sys::status::RUNTIME_ERROR as i32,
    }
}

fn main() {
    split_video_file("src/video.mp4".to_string()).unwrap();
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

        // Encode the images
        let num_frames = Path::new("ffmpeg-temp").read_dir().unwrap().count();

        std::process::Command::new("clear").status().unwrap();

        // begin timer
        let mut frame = 0;
        let mut stdout = std::io::stdout().into_raw_mode().unwrap();

        let mut images = Vec::with_capacity(num_frames);
        let scale_constant = 1.5;

        for i in 0..num_frames {
            let filename = format!("ffmpeg-temp/frame{}.ppm", i);
            let filepath = Path::new(&filename);
            let img = image::open(filepath).unwrap();
            let (width, height) = (
                (img.width() as f64 / scale_constant) as u32,
                (img.height() as f64 / scale_constant) as u32,
            );
            let img = img.resize(width, height, image::imageops::FilterType::Nearest);
            images.push(img);
            print!(
                "{}Loading images: {}/{}",
                termion::cursor::Goto(1, 1),
                i,
                num_frames
            );
        }

        for i in 0..num_frames {
            let start = std::time::Instant::now();
            match writeln!(stdout, "{}", termion::cursor::Goto(1, 1)) {
                Err(e) => panic!("{}stdout error: {}", termion::screen::ToMainScreen, e),
                Ok(_) => {}
            }

            // let filename_bytes = format!("ffmpeg-temp/frame{}.ppm\0", i).into_bytes();
            // let file_name = CStr::from_bytes_with_nul(&filename_bytes).unwrap();
            let img = &images[i];
            let image_bytes = img.to_rgb8().into_raw();
            let (width, height) = (img.width(), img.height());

            let status = sixel_encoder_encode_bytes(
                encoder,
                image_bytes.as_ptr() as *mut u8,
                width as i32,
                height as i32,
                sixel_sys::PixelFormat::RGB888,
                sixel_dither_get_palette(dither),
                sixel_sys::sixel_dither_get_num_of_palette_colors(dither),
            );

            // Check for errors
            match status {
                sixel_sys::status::OK => {
                    frame += 1;
                    let elapsed = start.elapsed();
                    let fps = 1 as f64 / elapsed.as_secs_f64();
                    println!("width: {}, height: {}, fps: {}", width, height, fps);
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

        // delete temp files
        std::fs::remove_dir_all("ffmpeg-temp").unwrap();

        // Clean up
        sixel_encoder_unref(encoder);
        sixel_dither_unref(dither);
        sixel_output_unref(output);
    }
}
