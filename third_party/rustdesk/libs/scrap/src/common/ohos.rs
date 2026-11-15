use crate::{Frame, Pixfmt, TraitCapturer, TraitPixelBuffer};
use std::{io, time::Duration};

/// Borrowed RGBA frame supplied by the HarmonyOS AVScreenCapture adapter.
pub struct PixelBuffer<'a> {
    data: &'a [u8],
    width: usize,
    height: usize,
    stride: Vec<usize>,
}

impl<'a> PixelBuffer<'a> {
    pub fn new(data: &'a [u8], width: usize, height: usize, stride: usize) -> Self {
        Self {
            data,
            width,
            height,
            stride: vec![stride],
        }
    }
}

impl TraitPixelBuffer for PixelBuffer<'_> {
    fn data(&self) -> &[u8] {
        self.data
    }
    fn width(&self) -> usize {
        self.width
    }
    fn height(&self) -> usize {
        self.height
    }
    fn stride(&self) -> Vec<usize> {
        self.stride.clone()
    }
    fn pixfmt(&self) -> Pixfmt {
        Pixfmt::RGBA
    }
}

/// Capture lifecycle is owned by ArkTS. This Rust type is intentionally a
/// non-blocking placeholder until the AVScreenCapture callback is registered.
pub struct Capturer;

impl Capturer {
    pub fn new(_display: Display) -> io::Result<Self> {
        Ok(Self)
    }
    pub fn width(&self) -> usize {
        0
    }
    pub fn height(&self) -> usize {
        0
    }
}

impl TraitCapturer for Capturer {
    fn frame<'a>(&'a mut self, _timeout: Duration) -> io::Result<Frame<'a>> {
        Err(io::ErrorKind::WouldBlock.into())
    }
}

#[derive(Clone)]
pub struct Display;

impl Display {
    pub fn primary() -> io::Result<Self> {
        Ok(Self)
    }
    pub fn all() -> io::Result<Vec<Self>> {
        Ok(vec![Self])
    }
    pub fn width(&self) -> usize {
        0
    }
    pub fn height(&self) -> usize {
        0
    }
    pub fn origin(&self) -> (i32, i32) {
        (0, 0)
    }
    pub fn is_online(&self) -> bool {
        true
    }
    pub fn is_primary(&self) -> bool {
        true
    }
    pub fn name(&self) -> String {
        "HarmonyOS".into()
    }
    pub fn refresh_size() {}
    pub fn fix_quality() -> u16 {
        1
    }
}

pub fn is_start() -> Option<bool> {
    Some(false)
}
