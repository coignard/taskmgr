// This file is part of Task Manager.
//
// Copyright (c) 2026  René Coignard <contact@renecoignard.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::io::Cursor;

use png::{BitDepth, ColorType, Decoder, DecodingError, Transformations};

use crate::geometry::Size;
use crate::graphics::{Color, TRANSPARENT};

const OPAQUE_FROM: u8 = 128;

#[derive(Debug)]
pub(crate) struct Image {
    size: Size,
    pixels: Vec<Color>,
}

impl Image {
    pub(crate) fn decode(png: &[u8]) -> Result<Self, DecodingError> {
        let mut decoder = Decoder::new(Cursor::new(png));
        decoder.set_transformations(Transformations::EXPAND | Transformations::ALPHA);
        let mut reader = decoder.read_info()?;
        let length = reader
            .output_buffer_size()
            .ok_or(DecodingError::LimitsExceeded)?;
        let mut buffer = vec![0; length];
        let info = reader.next_frame(&mut buffer)?;
        if (info.color_type, info.bit_depth) != (ColorType::Rgba, BitDepth::Eight) {
            return Err(DecodingError::LimitsExceeded);
        }
        let dimension =
            |pixels: u32| i32::try_from(pixels).map_err(|_| DecodingError::LimitsExceeded);
        let size = Size::new(dimension(info.width)?, dimension(info.height)?);
        let pixels = buffer[..info.buffer_size()]
            .as_chunks()
            .0
            .iter()
            .map(|&[red, green, blue, alpha]| {
                if alpha < OPAQUE_FROM {
                    TRANSPARENT
                } else {
                    [red, green, blue, u8::MAX]
                }
            })
            .collect();
        Ok(Self { size, pixels })
    }

    pub(crate) fn size(&self) -> Size {
        self.size
    }

    pub(crate) fn pixel(&self, x: i32, y: i32) -> Color {
        usize::try_from(y * self.size.width + x)
            .ok()
            .and_then(|index| self.pixels.get(index))
            .copied()
            .unwrap_or(TRANSPARENT)
    }
}
