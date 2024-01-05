extern crate sixel_sys;

use sixel_sys::{
    sixel_dither_get, sixel_dither_unref, sixel_encoder_create, sixel_encoder_encode,
    sixel_encoder_unref, sixel_output_create, sixel_output_unref,
};
use std::ffi::CStr;
use std::io::{self, Write};
use std::os::raw::c_void;
use std::ptr;
use std::slice;

unsafe extern "C" fn write_function(data: *mut i8, size: i32, _priv_data: *mut c_void) -> i32 {
    let data_slice = unsafe { slice::from_raw_parts(data as *const u8, size as usize) };
    io::stdout().write_all(data_slice).unwrap();
    sixel_sys::status::OK as i32
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

        // Encode the image
        let file_name = CStr::from_bytes_with_nul(b"src/image.png\0").unwrap();
        let status = sixel_encoder_encode(encoder, file_name.as_ptr());

        // Check for errors
        if status != sixel_sys::status::OK {
            eprintln!("Failed to encode image");
            // print error message in text
            let error_message = sixel_sys::sixel_helper_format_error(status);
            let message = CStr::from_ptr(error_message);
            eprintln!("Error: {}", message.to_str().unwrap());
        }

        // Clean up
        sixel_encoder_unref(encoder);
        sixel_dither_unref(dither);
        sixel_output_unref(output);
    }
}
