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

use crate::assets::FONT;
use crate::geometry::Rect;
use crate::graphics::{BLACK, Canvas, Color, DARK, FACE, Font, SHADOW, WHITE};
use crate::ui::Cues;

pub(crate) type Bevel = [(Color, Color); 2];

pub(crate) const RAISED: Bevel = [(FACE, DARK), (WHITE, SHADOW)];
pub(crate) const BUTTON: Bevel = [(WHITE, DARK), (FACE, SHADOW)];
pub(crate) const PUSHED: Bevel = [(DARK, WHITE), (SHADOW, FACE)];
pub(crate) const SUNKEN: Bevel = [(SHADOW, WHITE), (DARK, FACE)];
pub(crate) const ETCHED: Bevel = [(SHADOW, WHITE), (WHITE, SHADOW)];
pub(crate) const FLAT: Bevel = [(SHADOW, SHADOW), (FACE, FACE)];
pub(crate) const BOX: i32 = 13;
const CHECK: &str = "......# .....## #...### ##.###. #####.. .###... ..#....";

pub(crate) fn bevel(canvas: &mut Canvas, area: Rect, [outer, inner]: Bevel) {
    canvas.frame(area, outer.0, outer.1);
    canvas.frame(area.inset(1), inner.0, inner.1);
}

pub(crate) fn knob(canvas: &mut Canvas, area: Rect, look: Bevel) {
    canvas.fill(area, FACE);
    bevel(canvas, area, look);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ink {
    Plain(Color),
    Embossed,
}

pub(crate) fn label(canvas: &mut Canvas, text: &str, x: i32, top: i32, ink: Ink, underline: bool) {
    lettering(canvas, &FONT, text, x, top, ink, underline);
}

pub(crate) fn lettering(
    canvas: &mut Canvas,
    font: &Font,
    text: &str,
    x: i32,
    top: i32,
    ink: Ink,
    underline: bool,
) {
    let (plain, mnemonic) = mnemonic(text);
    let mut write = |dx: i32, color: Color| {
        font.draw(canvas, &plain, x + dx, top + dx, color);
        if let Some(index) = mnemonic.filter(|_| underline) {
            let left = font.width(&plain[..index]);
            let width = plain[index..]
                .chars()
                .next()
                .map_or(0, |key| font.advance(key));
            let baseline = top + font.height() - 2 + dx;
            canvas.fill(Rect::new(x + dx + left, baseline, width, 1), color);
        }
    };
    match ink {
        Ink::Plain(color) => write(0, color),
        Ink::Embossed => {
            write(1, WHITE);
            write(0, SHADOW);
        }
    }
}

pub(crate) fn mnemonic(text: &str) -> (String, Option<usize>) {
    let mut plain = String::with_capacity(text.len());
    let mut index = None;
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        match (character, character == '&') {
            (_, true) => match characters.next() {
                Some('&') => plain.push('&'),
                Some(marked) => {
                    index.get_or_insert(plain.len());
                    plain.push(marked);
                }
                None => {}
            },
            (character, false) => plain.push(character),
        }
    }
    (plain, index)
}

pub(crate) fn accelerator(text: &str) -> Option<char> {
    let (plain, index) = mnemonic(text);
    plain[index?..]
        .chars()
        .next()
        .map(|letter| letter.to_ascii_lowercase())
}

pub(crate) fn width(text: &str) -> i32 {
    FONT.width(&mnemonic(text).0)
}

pub(crate) fn focus(canvas: &mut Canvas, area: Rect, background: Color) {
    let mut dot = |x: i32, y: i32| {
        if (x - area.x + y - area.y) % 2 == 0 {
            let [red, green, blue, alpha] = canvas.pixel(x, y);
            let [r, g, b, _] = background;
            canvas.plot(x, y, [red ^ r, green ^ g, blue ^ b, alpha]);
        }
    };
    for x in area.x..area.right() {
        dot(x, area.y);
        dot(x, area.bottom() - 1);
    }
    for y in area.y + 1..area.bottom() - 1 {
        dot(area.x, y);
        dot(area.right() - 1, y);
    }
}

pub(crate) fn checkbox(canvas: &mut Canvas, x: i32, y: i32, checked: bool, enabled: bool) {
    let area = Rect::new(x, y, BOX, BOX);
    bevel(canvas, area, SUNKEN);
    canvas.fill(area.inset(2), if enabled { WHITE } else { FACE });
    if checked {
        let ink = if enabled { BLACK } else { SHADOW };
        canvas.stamp(x + 3, y + 3, CHECK, &[('#', ink)]);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Normal,
    Pushed,
    Disabled,
}

pub(crate) fn button(
    canvas: &mut Canvas,
    font: &Font,
    area: Rect,
    text: &str,
    state: State,
    cues: Cues,
    default: bool,
) {
    let pushed = state == State::Pushed;
    if default {
        canvas.frame(area, BLACK, BLACK);
        knob(canvas, area.inset(1), if pushed { FLAT } else { BUTTON });
    } else {
        knob(canvas, area, if pushed { PUSHED } else { BUTTON });
    }
    let shift = i32::from(pushed);
    let x = area.x + (area.width - font.width(&mnemonic(text).0)) / 2 + shift;
    let top = area.y + (area.height - font.height()) / 2 + shift;
    let ink = if state == State::Disabled {
        Ink::Embossed
    } else {
        Ink::Plain(BLACK)
    };
    lettering(canvas, font, text, x, top, ink, cues.keys);
    if cues.focus {
        focus(canvas, area.inset(4), FACE);
    }
}
