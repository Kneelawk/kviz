#![allow(dead_code)]

use std::ops::{Add, Index, IndexMut};

pub struct MultiSlice<T> {
    backing: Vec<Vec<T>>,
}

impl<T> MultiSlice<T> {
    pub fn new(backing: Vec<Vec<T>>) -> MultiSlice<T> {
        MultiSlice { backing }
    }

    pub fn len(&self) -> usize {
        self.backing.len()
    }

    pub fn get(&self, index: usize) -> Option<&[T]> {
        self.backing.get(index).map(|vec| &vec[..])
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut [T]> {
        self.backing.get_mut(index).map(|vec| &mut vec[..])
    }
}

impl<T> Index<usize> for MultiSlice<T> {
    type Output = [T];

    fn index(&self, index: usize) -> &Self::Output {
        &self.backing[index]
    }
}

impl<T> IndexMut<usize> for MultiSlice<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.backing[index]
    }
}

pub fn scale_byte(a: u8, b: u8) -> u8 {
    ((a as u16 * b as u16) >> 8) as u8
}

pub fn pixel(x: usize, y: usize, width: u32) -> usize {
    (y * (width as usize) + x) * 4
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct RGB {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl RGB {
    pub const ZERO: RGB = RGB {
        r: 0.0,
        g: 0.0,
        b: 0.0,
    };

    pub fn new(r: f32, g: f32, b: f32) -> RGB {
        RGB { r, g, b }
    }

    pub fn from_pixel(frame: &[u8], x: usize, y: usize, width: u32) -> RGB {
        let index = pixel(x, y, width);
        RGB {
            r: (frame[index + 1] as f32 + 0.5) / 256.0,
            g: (frame[index + 2] as f32 + 0.5) / 256.0,
            b: (frame[index + 3] as f32 + 0.5) / 256.0,
        }
    }

    pub fn write_pixel(&self, frame: &mut [u8], x: usize, y: usize, width: u32) {
        let index = pixel(x, y, width);

        frame[index] = 0xFF;
        frame[index + 1] = (self.r * 256.0) as u8;
        frame[index + 2] = (self.g * 256.0) as u8;
        frame[index + 3] = (self.b * 256.0) as u8;
    }

    pub fn scale(mut self, scale: f32) -> RGB {
        self.r *= scale;
        self.g *= scale;
        self.b *= scale;

        self
    }

    pub fn scale_mut(&mut self, scale: f32) {
        self.r *= scale;
        self.g *= scale;
        self.b *= scale;
    }
}

impl Add for RGB {
    type Output = RGB;

    fn add(self, rhs: Self) -> Self::Output {
        RGB {
            r: self.r + rhs.r,
            g: self.g + rhs.g,
            b: self.b + rhs.b,
        }
    }
}
