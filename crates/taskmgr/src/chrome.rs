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

use crate::assets::{BOLD, FONT, ICON};
use crate::controls::{self, BUTTON, Ink, PUSHED, RAISED};
use crate::geometry::{Point, Rect, Size};
use crate::graphics::{
    BLACK, Canvas, Color, FACE, GRADIENT, HIGHLIGHT, INACTIVE_GRADIENT, SHADOW, TRANSPARENT, WHITE,
};
use crate::menu::Menu;
use crate::ui::{Edge, Target, Ui};

pub(crate) const FRAME: i32 = 4;
pub(crate) const CAPTION: i32 = 18;
const BAR: i32 = 19;
const STATUS: i32 = 20;
const CORNER: i32 = 18;
const KNOB: Size = Size::new(16, 14);
const MINIMIZE: &str = "###### ######";
const MAXIMIZE: &str =
    "######### ######### #.......# #.......# #.......# #.......# #.......# #.......# #########";
const RESTORE: &str =
    "...###### ...###### ...#....# ######..# ######..# #....#### #....#... #....#... ######...";
const CLOSE: &str = "##....## .##..##. ..####.. ...##... ..####.. .##..##. ##....##";
const HELP: &str = ".####. ##..## ##..## ...##. ..##.. ..##.. ...... ..##.. ..##..";
const GRIP: &str = "...........w ..........wg .........wgg ........wgg. .......wgg.w ......wgg.wg .....wgg.wgg ....wgg.wgg. ...wgg.wgg.w ..wgg.wgg.wg .wgg.wgg.wgg wgg.wgg.wgg.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Title {
    Plain,
    Hot,
    Open,
}

pub(crate) fn client(size: Size) -> Rect {
    Rect::sides(
        FRAME,
        FRAME + CAPTION + BAR,
        size.width - FRAME,
        size.height - FRAME,
    )
}

pub(crate) fn status_area(size: Size) -> Rect {
    let client = client(size);
    Rect::new(client.x, client.bottom() - STATUS, client.width, STATUS)
}

pub(crate) fn frame(ui: &mut Ui, size: Size, sizable: bool) {
    let area = Rect::new(0, 0, size.width, size.height);
    controls::bevel(&mut ui.canvas, area, RAISED);
    if sizable {
        for band in [
            Rect::new(0, 0, size.width, FRAME),
            Rect::new(0, size.height - FRAME, size.width, FRAME),
            Rect::new(0, 0, FRAME, size.height),
            Rect::new(size.width - FRAME, 0, FRAME, size.height),
        ] {
            ui.target(band, Target::Frame);
        }
    }
}

pub(crate) fn edge(size: Size, point: Point) -> Edge {
    let zone = |at: i32, length: i32| i32::from(at >= length - CORNER) - i32::from(at < CORNER);
    Edge {
        x: zone(point.x, size.width),
        y: zone(point.y, size.height),
    }
}

pub(crate) fn stretch(size: Size, edge: Edge, by: Point, least: Size) -> Rect {
    let span = |zone: i32, length: i32, by: i32, least: i32| match zone {
        -1 => {
            let start = by.min(length - least);
            (start, length - start)
        }
        1 => (0, (length + by).max(least)),
        _ => (0, length),
    };
    let (x, width) = span(edge.x, size.width, by.x, least.width);
    let (y, height) = span(edge.y, size.height, by.y, least.height);
    Rect::new(x, y, width, height)
}

pub(crate) fn outline(layers: &[(Point, &Canvas)], area: Rect, thickness: i32) -> Canvas {
    let mut canvas = Canvas::new(area.size(), TRANSPARENT);
    let inner = area.inset(thickness);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if inner.contains(Point::new(x, y)) {
                continue;
            }
            let [red, green, blue, _] = layers
                .iter()
                .rev()
                .map(|(at, canvas)| canvas.pixel(x - at.x, y - at.y))
                .find(|&color| color != TRANSPARENT)
                .unwrap_or(WHITE);
            let color = if (x + y) % 2 == 0 {
                [!red, !green, !blue, u8::MAX]
            } else {
                [red, green, blue, u8::MAX]
            };
            canvas.plot(x - area.x, y - area.y, color);
        }
    }
    canvas
}

pub(crate) struct Knob {
    pub(crate) target: Target,
    pub(crate) glyph: &'static str,
    pub(crate) at: (i32, i32),
    pub(crate) enabled: bool,
}

pub(crate) const fn knobs(zoomed: bool) -> [Knob; 3] {
    [
        Knob {
            target: Target::Minimize,
            glyph: MINIMIZE,
            at: (4, 9),
            enabled: true,
        },
        Knob {
            target: Target::Zoom,
            glyph: if zoomed { RESTORE } else { MAXIMIZE },
            at: (3, 2),
            enabled: true,
        },
        Knob {
            target: Target::Close,
            glyph: CLOSE,
            at: (4, 3),
            enabled: true,
        },
    ]
}

pub(crate) const fn knob(target: Target, help: bool, enabled: bool) -> Knob {
    let (glyph, at) = if help {
        (HELP, (5, 2))
    } else {
        (CLOSE, (4, 3))
    };
    Knob {
        target,
        glyph,
        at,
        enabled,
    }
}

pub(crate) struct Caption<'a> {
    pub(crate) area: Rect,
    pub(crate) title: &'a str,
    pub(crate) active: bool,
    pub(crate) icon: bool,
    pub(crate) drag: Target,
}

pub(crate) fn caption(ui: &mut Ui, caption: &Caption<'_>, knobs: &[Knob]) {
    let Caption {
        area,
        title,
        active,
        icon,
        drag,
    } = *caption;
    let close = Rect::new(
        area.right() - 2 - KNOB.width,
        area.y + 2,
        KNOB.width,
        KNOB.height,
    );
    let count = i32::try_from(knobs.len()).unwrap_or(1) - 1;
    let first = close.x - 2 - KNOB.width * count;
    let (start, end) = if active {
        (HIGHLIGHT, GRADIENT)
    } else {
        (SHADOW, INACTIVE_GRADIENT)
    };
    let from = if icon {
        area.x + 2 + ICON.size().width
    } else {
        area.x
    };
    let edge = if count > 0 { first } else { close.x };
    let span = (edge - from - 1).max(1);
    for x in area.x..area.right() {
        let step = (x - from).clamp(0, span);
        let color = blend(start, end, step, span);
        ui.canvas.fill(Rect::new(x, area.y, 1, area.height), color);
    }
    ui.target(area, drag);
    if icon {
        ui.canvas.draw(&ICON, area.x + 2, area.y + 1);
    }
    let left = from + 2;
    let text = BOLD.ellipsized(title, edge - left - 2);
    let top = area.y + (area.height - BOLD.height()) / 2;
    BOLD.draw(
        &mut ui.canvas,
        &text,
        left,
        top,
        if active { WHITE } else { FACE },
    );
    for (index, knob) in (0..).zip(knobs) {
        let rect = if index == count {
            close
        } else {
            Rect::new(first + KNOB.width * index, close.y, KNOB.width, KNOB.height)
        };
        let pushed = knob.enabled && ui.held(knob.target);
        controls::knob(&mut ui.canvas, rect, if pushed { PUSHED } else { BUTTON });
        let shift = i32::from(pushed);
        let (x, y) = (rect.x + knob.at.0 + shift, rect.y + knob.at.1 + shift);
        if knob.enabled {
            ui.canvas.stamp(x, y, knob.glyph, &[('#', BLACK)]);
            ui.target(rect, knob.target);
        } else {
            ui.canvas.stamp(x + 1, y + 1, knob.glyph, &[('#', WHITE)]);
            ui.canvas.stamp(x, y, knob.glyph, &[('#', SHADOW)]);
        }
    }
}

pub(crate) fn titles(menus: &[Menu]) -> impl Iterator<Item = Rect> + '_ {
    menus.iter().scan(FRAME, |left, menu| {
        let width = controls::width(menu.title) + 12;
        let title = Rect::new(*left, FRAME + CAPTION + 1, width, BAR - 1);
        *left += width;
        Some(title)
    })
}

pub(crate) fn menu_bar(
    ui: &mut Ui,
    menus: &[Menu],
    look: impl Fn(usize) -> Title,
    active: bool,
    underline: bool,
) {
    for (index, (menu, title)) in menus.iter().zip(titles(menus)).enumerate() {
        let state = look(index);
        let shift = i32::from(state == Title::Open);
        match state {
            Title::Plain => {}
            Title::Hot => ui.canvas.frame(title, WHITE, SHADOW),
            Title::Open => ui.canvas.frame(title, SHADOW, WHITE),
        }
        let color = if active { BLACK } else { SHADOW };
        let top = title.y + (title.height - FONT.height()) / 2;
        controls::label(
            &mut ui.canvas,
            menu.title,
            title.x + 6 + shift,
            top + shift,
            Ink::Plain(color),
            underline,
        );
        ui.target(title, Target::Title(index));
    }
    let client = client(ui.canvas.size());
    ui.canvas
        .fill(Rect::new(client.x, client.y + 1, client.width, 1), SHADOW);
    ui.canvas
        .fill(Rect::new(client.x, client.y + 2, client.width, 1), WHITE);
}

pub(crate) fn status_bar(
    ui: &mut Ui,
    rights: &[i32],
    parts: &[&str],
    simple: Option<&str>,
    sizable: bool,
) {
    let area = status_area(ui.canvas.size());
    let line = area.shrink(0, 2, 0, 0);
    if let Some(text) = simple {
        let room = line.width - if sizable { 16 } else { 0 };
        let text = FONT.ellipsized(text, room);
        FONT.draw(&mut ui.canvas, &text, line.x + 2, line.y + 2, BLACK);
    } else {
        let mut left = line.x;
        for (index, text) in parts.iter().enumerate() {
            let right = rights
                .get(index)
                .map_or(line.right(), |right| line.x + right);
            let pane = Rect::sides(left, line.y, right, line.bottom());
            ui.canvas.frame(pane, SHADOW, WHITE);
            ui.canvas.clipped(pane.inset(1), |canvas| {
                FONT.draw(canvas, text, pane.x + 2, pane.y + 2, BLACK);
            });
            left = right + 2;
        }
    }
    if sizable {
        let grip = Rect::new(area.right() - 13, area.bottom() - 13, 13, 13);
        ui.canvas.fill(grip, FACE);
        ui.canvas
            .stamp(grip.x, grip.y, GRIP, &[('w', WHITE), ('g', SHADOW)]);
        ui.target(grip, Target::Frame);
    }
}

fn blend(start: Color, end: Color, step: i32, span: i32) -> Color {
    let mix = |from: u8, to: u8| {
        let value = i32::from(from) + (i32::from(to) - i32::from(from)) * step / span;
        u8::try_from(value).unwrap_or(u8::MAX)
    };
    [
        mix(start[0], end[0]),
        mix(start[1], end[1]),
        mix(start[2], end[2]),
        u8::MAX,
    ]
}
