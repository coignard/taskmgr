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

use std::borrow::Cow;

use skrifa::bitmap::{BitmapData, BitmapGlyph, BitmapStrike, BitmapStrikes};
use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::{GlyphMetrics, Metrics};
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::graphics::{Canvas, Color};

const ELLIPSIS: &str = "...";

pub(crate) struct Font {
    charmap: Charmap<'static>,
    metrics: GlyphMetrics<'static>,
    strike: BitmapStrike<'static>,
    units_per_em: i32,
    pixels_per_em: i32,
    ascent: i32,
    height: i32,
}

impl Font {
    pub(crate) fn parse(data: &'static [u8], pixels_per_em: u16) -> Option<Self> {
        let font = FontRef::new(data).ok()?;
        let size = f32::from(pixels_per_em);
        let strike = BitmapStrikes::new(&font)
            .iter()
            .find(|strike| strike.ppem().total_cmp(&size).is_eq())?;
        let Metrics {
            units_per_em,
            ascent,
            descent,
            ..
        } = font.metrics(Size::unscaled(), LocationRef::default());
        let units_per_em = i32::from(units_per_em);
        let pixels_per_em = i32::from(pixels_per_em);
        let scale = |units: f32| pixels(units) * pixels_per_em / units_per_em;
        Some(Self {
            charmap: font.charmap(),
            metrics: font.glyph_metrics(Size::unscaled(), LocationRef::default()),
            strike,
            units_per_em,
            pixels_per_em,
            ascent: scale(ascent),
            height: scale(ascent - descent),
        })
    }

    #[must_use]
    pub(crate) const fn cell(self, ascent: i32, height: i32) -> Self {
        Self {
            ascent,
            height,
            ..self
        }
    }

    pub(crate) fn height(&self) -> i32 {
        self.height
    }

    pub(crate) fn width(&self, text: &str) -> i32 {
        text.chars().map(|character| self.advance(character)).sum()
    }

    pub(crate) fn ellipsized<'a>(&self, text: &'a str, width: i32) -> Cow<'a, str> {
        if self.width(text) <= width {
            return Cow::Borrowed(text);
        }
        let room = width - self.width(ELLIPSIS);
        let mut used = 0;
        let kept: String = text
            .chars()
            .take_while(|&character| {
                used += self.advance(character);
                used <= room
            })
            .collect();
        Cow::Owned(kept + ELLIPSIS)
    }

    pub(crate) fn draw(&self, canvas: &mut Canvas, text: &str, x: i32, top: i32, color: Color) {
        let mut pen = x;
        for character in text.chars() {
            let glyph = self.glyph(character);
            if let Some(bitmap) = self.strike.get(glyph) {
                draw_bitmap(canvas, &bitmap, pen, top + self.ascent, color);
            }
            pen += self.glyph_advance(glyph);
        }
    }

    pub(crate) fn advance(&self, character: char) -> i32 {
        self.glyph_advance(self.glyph(character))
    }

    fn glyph(&self, character: char) -> GlyphId {
        self.charmap.map(character).unwrap_or(GlyphId::NOTDEF)
    }

    fn glyph_advance(&self, glyph: GlyphId) -> i32 {
        let units = pixels(self.metrics.advance_width(glyph).unwrap_or_default());
        (units * self.pixels_per_em + self.units_per_em / 2) / self.units_per_em
    }
}

fn draw_bitmap(canvas: &mut Canvas, bitmap: &BitmapGlyph<'_>, x: i32, baseline: i32, color: Color) {
    let BitmapData::Mask(mask) = &bitmap.data else {
        return;
    };
    let (Ok(mask), Ok(width)) = (
        mask.decode(bitmap.width, bitmap.height),
        usize::try_from(bitmap.width),
    ) else {
        return;
    };
    let left = x + pixels(bitmap.inner_bearing_x);
    let top = baseline - pixels(bitmap.inner_bearing_y);
    for (row, line) in (0..).zip(mask.chunks(width.max(1))) {
        for (column, &coverage) in (0..).zip(line) {
            if coverage != 0 {
                canvas.plot(left + column, top + row, color);
            }
        }
    }
}

#[expect(clippy::cast_possible_truncation)]
fn pixels(length: f32) -> i32 {
    length.round() as i32
}

#[cfg(test)]
mod tests {
    use crate::assets::{BOLD, FONT};
    use crate::geometry::Size;
    use crate::graphics::{BLACK, Canvas, TRANSPARENT};

    fn rows(text: &str, font: &super::Font) -> Vec<String> {
        let mut canvas = Canvas::new(Size::new(font.width(text), font.height()), TRANSPARENT);
        font.draw(&mut canvas, text, 0, 0, BLACK);
        (0..canvas.size().height)
            .map(|y| {
                (0..canvas.size().width)
                    .map(|x| {
                        if canvas.pixel(x, y) == BLACK {
                            '#'
                        } else {
                            '.'
                        }
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn draws_tahoma_as_windows_2000_did() {
        assert_eq!(
            rows("File", &FONT)[2..11],
            [
                "........#.......",
                "#####.#.#.......",
                "#.......#.......",
                "#.....#.#..###..",
                "#####.#.#.#...#.",
                "#.....#.#.#####.",
                "#.....#.#.#.....",
                "#.....#.#.#...#.",
                "#.....#.#..###..",
            ]
        );
        assert_eq!(rows("W", &BOLD)[6], ".##.##.##..");
    }

    #[test]
    fn ellipsizes_what_does_not_fit() {
        assert_eq!(FONT.ellipsized("Processes", 100), "Processes");
        let short = FONT.ellipsized("Processes", 30);
        assert!(short.ends_with("...") && FONT.width(&short) <= 30);
    }

    #[test]
    fn covers_english_and_russian_text() {
        let characters = (' '..='~').chain('А'..='я').chain("Ёё«»„“”‘’—–…№".chars());
        for character in characters {
            assert!(FONT.charmap.map(character).is_some(), "{character:?}");
        }
    }
}
