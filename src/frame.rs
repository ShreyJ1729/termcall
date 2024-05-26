use opencv::core::{Mat, Point3_, Size};
use opencv::{imgcodecs, imgproc, prelude::*};

const ASCII_CHAR_H_OVER_W: f64 = 2.25;

pub struct Frame {
    data: Mat,
}

impl Frame {
    // Purposefully lightweight to allow for multiple struct instances
    pub fn new() -> Frame {
        let data = Mat::default();
        Frame { data }
    }

    pub fn get_ref(&self) -> &Mat {
        &self.data
    }

    pub fn get_mut_ref(&mut self) -> &mut Mat {
        &mut self.data
    }

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut buf = opencv::types::VectorOfu8::new();
        imgcodecs::imencode(".jpg", &self.data, &mut buf, &opencv::core::Vector::new()).unwrap();
        buf.to_vec()
    }

    // Resizes a Mat to the specified width and height
    pub fn resize_frame(&mut self, new_width: f64, new_height: f64, preserve_aspect_ratio: bool) {
        let (orig_width, orig_height) = (
            self.data.cols() as f64 * ASCII_CHAR_H_OVER_W,
            self.data.rows() as f64,
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

        let frame_read = self.data.clone();

        imgproc::resize(
            &frame_read,
            &mut self.data,
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
        let data = self.data.data_typed_mut::<Point3_<u8>>().unwrap();

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
        unsafe { self.data.set_data(data_ptr) }
    }

    pub fn get_frame(&self) -> &Mat {
        &self.data
    }

    pub fn load_bytes(&mut self, bytes: Vec<u8>) {
        let mat = imgcodecs::imdecode(
            &opencv::types::VectorOfu8::from(bytes),
            imgcodecs::IMREAD_COLOR,
        )
        .unwrap();

        self.data = mat;
    }

    pub fn load_mat(&mut self, mat: &Mat) {
        self.data = mat.clone();
    }

    pub fn get_frame_mirrored(&mut self) -> &Mat {
        let orig_frame = self.data.clone();
        opencv::core::flip(&orig_frame, &mut self.data, 1).unwrap();
        &self.data
    }

    pub fn width(&self) -> i32 {
        self.data.cols()
    }

    pub fn height(&self) -> i32 {
        self.data.rows()
    }

    pub fn num_pixels(&self) -> i32 {
        self.data.total() as i32
    }
}
