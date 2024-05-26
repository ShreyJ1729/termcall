use opencv::core::{Mat, Point3_, Size};
use opencv::videoio::VideoCapture;
use opencv::{imgcodecs, imgproc, prelude::*, videoio};

const ASCII_CHAR_H_OVER_W: f64 = 2.25;

pub struct FrameWriter {
    frame: Mat,
}

impl FrameWriter {
    // Purposefully lightweight to allow for multiple struct instances
    pub fn new() -> FrameWriter {
        let frame = Mat::default();
        FrameWriter { frame }
    }

    // Resizes a Mat to the specified width and height
    pub fn resize_frame(&mut self, new_width: f64, new_height: f64, preserve_aspect_ratio: bool) {
        let (orig_width, orig_height) = (
            self.frame.cols() as f64 * ASCII_CHAR_H_OVER_W,
            self.frame.rows() as f64,
        );
        let orig_ratio = orig_width / orig_height;

        let new_size = match preserve_aspect_ratio {
            true => {
                let new_ratio = new_width / new_height;
                if new_ratio > orig_ratio {
                    Size {
                        width: (new_height * orig_ratio) as i32,
                        height: new_height as i32,
                    }
                } else {
                    Size {
                        width: new_width as i32,
                        height: (new_width / orig_ratio) as i32,
                    }
                }
            }
            false => Size {
                width: new_width as i32,
                height: new_height as i32,
            },
        };

        let frame_read = self.frame.clone();

        imgproc::resize(
            &frame_read,
            &mut self.frame,
            new_size,
            0.0,
            0.0,
            opencv::imgproc::INTER_LINEAR,
        )
        .unwrap();
    }

    // changes color depth by rounding each color channel to the nearest multiple of 255/new_colors_per_channel
    // CHaing color depth helps reduce latency since less bytes must be written to terminal per cycle
    pub fn change_color_depth(&mut self, colors_per_channel: u8) {
        let data = self.frame.data_typed_mut::<Point3_<u8>>().unwrap();

        let multiple = 255 / colors_per_channel;

        // rounds each r, g, b to nearest multiple of 255/new_colors_per_channel and clamps to 0-255
        let convert = |rgb_value: u8| {
            ((rgb_value as f64 / multiple as f64).round() * multiple as f64).clamp(0.0, 255.0) as u8
        };

        // convert each pixel
        for pixel in &mut *data {
            pixel.x = convert(pixel.x);
            pixel.y = convert(pixel.y);
            pixel.z = convert(pixel.z);
        }

        // set data back to frame
        let data_ptr: *const u8 = data.as_ptr() as *const u8;
        unsafe { self.frame.set_data(data_ptr) }
    }

    pub fn get_frame(&self) -> &Mat {
        &self.frame
    }

    pub fn load_bytes(&mut self, bytes: Vec<u8>) {
        let mat = imgcodecs::imdecode(
            &opencv::types::VectorOfu8::from(bytes),
            imgcodecs::IMREAD_COLOR,
        )
        .unwrap();

        self.frame = mat;
    }

    pub fn get_frame_mirrored(&mut self) -> &Mat {
        let orig_frame = self.frame.clone();
        opencv::core::flip(&orig_frame, &mut self.frame, 1).unwrap();
        &self.frame
    }

    pub fn get_frame_width(&self) -> i32 {
        self.frame.cols()
    }

    pub fn get_frame_height(&self) -> i32 {
        self.frame.rows()
    }

    pub fn get_frame_num_pixels(&self) -> i32 {
        self.frame.total() as i32
    }
}
