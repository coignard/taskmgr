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

use crate::controls::{self, FLAT, RAISED, State};
use crate::geometry::Rect;
use crate::graphics::{BLACK, Canvas, FACE, SHADOW, WHITE};
use crate::ui::{Target, Ui};

pub(crate) const BREADTH: i32 = 16;
const SMALLEST: i32 = 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Back,
    Forth,
    PageBack,
    PageForth,
    Thumb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bar {
    pub(crate) area: Rect,
    pub(crate) vertical: bool,
    pub(crate) position: i32,
    pub(crate) page: i32,
    pub(crate) total: i32,
}

impl Bar {
    fn length(&self, rect: Rect) -> i32 {
        if self.vertical {
            rect.height
        } else {
            rect.width
        }
    }

    fn along(&self, rect: Rect) -> i32 {
        if self.vertical { rect.y } else { rect.x }
    }

    fn span(&self, start: i32, length: i32) -> Rect {
        if self.vertical {
            Rect::new(self.area.x, start, BREADTH, length)
        } else {
            Rect::new(start, self.area.y, length, BREADTH)
        }
    }

    fn track(&self) -> Rect {
        let start = self.along(self.area);
        self.span(start + BREADTH, self.length(self.area) - 2 * BREADTH)
    }

    pub(crate) fn thumb(&self) -> Option<Rect> {
        let track = self.track();
        let room = self.length(track);
        let range = self.total - self.page;
        let length = (room * self.page / self.total.max(1)).max(SMALLEST);
        (range > 0 && length < room).then(|| {
            let offset = (room - length) * self.position.clamp(0, range) / range;
            self.span(self.along(track) + offset, length)
        })
    }

    pub(crate) fn dragged(&self, start: i32) -> i32 {
        let Some(thumb) = self.thumb() else {
            return 0;
        };
        let room = self.length(self.track()) - self.length(thumb);
        let range = self.total - self.page;
        ((start - self.along(self.track())) * range + room / 2) / room.max(1)
    }

    pub(crate) fn draw(&self, ui: &mut Ui) {
        let start = self.along(self.area);
        let end = start + self.length(self.area) - BREADTH;
        let track = self.track();
        for y in track.y..track.bottom() {
            for x in track.x..track.right() {
                ui.canvas
                    .plot(x, y, if (x + y) % 2 == 0 { WHITE } else { FACE });
            }
        }
        ui.target(track, Target::Scroll(self.vertical, Part::PageForth));
        if let Some(thumb) = self.thumb() {
            let before = self.span(self.along(track), self.along(thumb) - self.along(track));
            ui.target(before, Target::Scroll(self.vertical, Part::PageBack));
            controls::knob(&mut ui.canvas, thumb, RAISED);
            ui.target(thumb, Target::Scroll(self.vertical, Part::Thumb));
        }
        for (forth, (at, part)) in [(false, (start, Part::Back)), (true, (end, Part::Forth))] {
            let button = self.span(at, BREADTH);
            let target = Target::Scroll(self.vertical, part);
            let state = if ui.held(target) {
                State::Pushed
            } else {
                State::Normal
            };
            arrow(&mut ui.canvas, button, self.vertical, forth, state);
            ui.target(button, target);
        }
    }
}

pub(crate) fn arrow(canvas: &mut Canvas, button: Rect, vertical: bool, forth: bool, state: State) {
    let pushed = state == State::Pushed;
    controls::knob(canvas, button, if pushed { FLAT } else { RAISED });
    let side = button.width.min(button.height);
    let depth = side / 4;
    let cross = (side - 2 * depth + 1) / 2;
    let along = (side - depth) / 2;
    let shift = i32::from(pushed);
    let x = button.x + (button.width - side) / 2 + shift;
    let y = button.y + (button.height - side) / 2 + shift;
    let mut paint = |offset: i32, color| {
        for step in 0..depth {
            let start = if forth { step } else { depth - 1 - step };
            let length = 2 * (depth - start) - 1;
            let row = if vertical {
                Rect::new(
                    x + cross + start + offset,
                    y + along + step + offset,
                    length,
                    1,
                )
            } else {
                Rect::new(
                    x + along + step + offset,
                    y + cross + start + offset,
                    1,
                    length,
                )
            };
            canvas.fill(row, color);
        }
    };
    if state == State::Disabled {
        paint(1, WHITE);
        paint(0, SHADOW);
    } else {
        paint(0, BLACK);
    }
}
