use crossterm::style::{Color, Print, SetBackgroundColor};
use image::{ImageBuffer, Rgb};
use simple_log::error;
use std::io::Write;

const ASCII_CHAR_H_OVER_W: f64 = 2.25;

pub struct Frame {
    data: ImageBuffer<Rgb<u8>, Vec<u8>>,
}

impl Frame {
    pub fn new() -> Frame {
        let data = ImageBuffer::new(1, 1);
        Frame { data }
    }

    pub fn get_ref(&self) -> &ImageBuffer<Rgb<u8>, Vec<u8>> {
        &self.data
    }

    pub fn get_mut_ref(&mut self) -> &mut ImageBuffer<Rgb<u8>, Vec<u8>> {
        &mut self.data
    }

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        for pixel in self.data.pixels() {
            buf.push(pixel[0]);
            buf.push(pixel[1]);
            buf.push(pixel[2]);
        }
        buf.to_vec()
    }

    // Resizes a Mat to the specified width and height
    pub fn resize_frame(
        &mut self,
        new_width: f64,
        new_height: f64,
        preserve_aspect_ratio: bool,
    ) -> anyhow::Result<()> {
        let (orig_width, orig_height) = (
            self.data.width() as f64 * ASCII_CHAR_H_OVER_W,
            self.data.height() as f64,
        );
        let orig_ratio = orig_width / orig_height;

        let new_size = match preserve_aspect_ratio {
            true => {
                let new_ratio = new_width / new_height;
                if new_ratio > orig_ratio {
                    ((new_height * orig_ratio) as i32, new_height as i32)
                } else {
                    (new_width as i32, (new_width / orig_ratio) as i32)
                }
            }
            false => (new_width as i32, new_height as i32),
        };

        let frame_read = self.data.clone();

        // resize frame to new size
        self.data = ImageBuffer::new(new_size.0 as u32, new_size.1 as u32);
        image::imageops::resize(
            &frame_read,
            new_size.0 as u32,
            new_size.1 as u32,
            image::imageops::FilterType::Nearest,
        );

        Ok(())
    }

    pub fn write_to_terminal(&mut self) {
        let frame = self.get_ref();
        let data = frame.pixels();
        let prev_color = Color::Rgb { r: 0, g: 0, b: 0 };
        let mut out = std::io::stdout();

        write!(out, "{}", crossterm::cursor::MoveTo(0, 0)).unwrap();
        for (i, pixel) in data.enumerate() {
            if (i % frame.width() as usize == 0) && i != 0 {
                write!(out, "\n\r").unwrap();
            }

            let r = pixel[0];
            let g = pixel[1];
            let b = pixel[2];
            let color = Color::Rgb { r, g, b };

            if color != prev_color {
                crossterm::execute!(out, SetBackgroundColor(color)).unwrap();
            }

            crossterm::execute!(out, Print(" ")).unwrap();
        }

        crossterm::execute!(out, SetBackgroundColor(Color::Reset)).unwrap();
    }

    pub fn load_bytes(&mut self, bytes: Vec<u8>) {
        self.data = ImageBuffer::from_raw(self.data.width(), self.data.height(), bytes).unwrap();
    }

    pub fn width(&self) -> i32 {
        self.data.width() as i32
    }

    pub fn height(&self) -> i32 {
        self.data.height() as i32
    }

    pub fn num_pixels(&self) -> i32 {
        (self.data.width() * self.data.height()) as i32
    }
}
