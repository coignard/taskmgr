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

use std::fmt;

use crate::geometry::{Point, Rect, Size};
use crate::graphics::{Color, Image, TRANSPARENT};

#[derive(Clone, PartialEq, Eq)]
pub struct Canvas {
    size: Size,
    pixels: Vec<Color>,
    clip: Rect,
}

impl Canvas {
    pub(crate) fn new(size: Size, color: Color) -> Self {
        let area = usize::try_from(size.width * size.height).unwrap_or_default();
        Self {
            size,
            pixels: vec![color; area],
            clip: Rect::new(0, 0, size.width, size.height),
        }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn rgba(&self) -> &[u8] {
        self.pixels.as_flattened()
    }

    pub fn pixel(&self, x: i32, y: i32) -> Color {
        self.index(x, y)
            .map_or(TRANSPARENT, |index| self.pixels[index])
    }

    pub(crate) fn plot(&mut self, x: i32, y: i32, color: Color) {
        if self.clip.contains(Point::new(x, y))
            && let Some(index) = self.index(x, y)
        {
            self.pixels[index] = color;
        }
    }

    pub(crate) fn fade(&mut self) {
        for pixel in &mut self.pixels {
            *pixel = pixel.map(|channel| channel / 2);
        }
    }

    pub(crate) fn invert(&mut self, area: Rect) {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let [red, green, blue, alpha] = self.pixel(x, y);
                self.plot(x, y, [!red, !green, !blue, alpha]);
            }
        }
    }

    pub(crate) fn fill(&mut self, area: Rect, color: Color) {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                self.plot(x, y, color);
            }
        }
    }

    pub(crate) fn frame(&mut self, area: Rect, top_left: Color, bottom_right: Color) {
        let Rect {
            x,
            y,
            width,
            height,
        } = area;
        self.fill(Rect::new(x, y, width - 1, 1), top_left);
        self.fill(Rect::new(x, y, 1, height - 1), top_left);
        self.fill(Rect::new(area.right() - 1, y, 1, height), bottom_right);
        self.fill(Rect::new(x, area.bottom() - 1, width, 1), bottom_right);
    }

    pub(crate) fn stamp(&mut self, x: i32, y: i32, glyph: &str, inks: &[(char, Color)]) {
        for (row, line) in (0..).zip(glyph.split(' ')) {
            for (column, pixel) in (0..).zip(line.chars()) {
                if let Some(&(_, color)) = inks.iter().find(|(key, _)| *key == pixel) {
                    self.plot(x + column, y + row, color);
                }
            }
        }
    }

    pub(crate) fn draw(&mut self, image: &Image, x: i32, y: i32) {
        let size = image.size();
        for row in 0..size.height {
            for column in 0..size.width {
                let color = image.pixel(column, row);
                if color != TRANSPARENT {
                    self.plot(x + column, y + row, color);
                }
            }
        }
    }

    pub(crate) fn clipped(&mut self, area: Rect, draw: impl FnOnce(&mut Self)) {
        let outer = self.clip;
        self.clip = area.intersect(outer);
        draw(self);
        self.clip = outer;
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let inside = (0..self.size.width).contains(&x) && (0..self.size.height).contains(&y);
        inside
            .then(|| usize::try_from(y * self.size.width + x).ok())
            .flatten()
    }
}

impl fmt::Debug for Canvas {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Canvas")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::{BLACK, WHITE};

    #[test]
    fn frames_give_corners_to_the_bottom_right() {
        let mut canvas = Canvas::new(Size::new(3, 3), TRANSPARENT);
        canvas.frame(Rect::new(0, 0, 3, 3), WHITE, BLACK);
        assert_eq!(canvas.pixel(0, 0), WHITE);
        assert_eq!(canvas.pixel(2, 0), BLACK);
        assert_eq!(canvas.pixel(0, 2), BLACK);
        assert_eq!(canvas.pixel(1, 1), TRANSPARENT);
    }

    #[test]
    fn clipping_keeps_drawing_inside() {
        let mut canvas = Canvas::new(Size::new(4, 1), TRANSPARENT);
        canvas.clipped(Rect::new(1, 0, 2, 1), |canvas| {
            canvas.fill(Rect::new(0, 0, 4, 1), BLACK);
        });
        canvas.plot(3, 0, WHITE);
        let pixels: Vec<Color> = (0..4).map(|x| canvas.pixel(x, 0)).collect();
        assert_eq!(pixels, [TRANSPARENT, BLACK, BLACK, WHITE]);
    }
}
