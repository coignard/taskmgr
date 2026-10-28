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
use crate::controls::{self, ETCHED, SUNKEN};
use crate::geometry::Rect;
use crate::graphics::{BLACK, Canvas, Color, FACE, GREEN, GRID, RED, YELLOW};

const GAP: i32 = 15;
const TITLES: [(&str, &str); 2] = [
    ("CPU Usage", "CPU Usage History"),
    ("MEM Usage", "Memory Usage History"),
];
const METER: i32 = 90;
const BLOCK: i32 = 63;
const LINES: [[i32; 3]; 2] = [[16, 31, 45], [16, 31, 46]];
const GRID_STEP: usize = 12;
const SAMPLE: i32 = 2;
const DIM: Color = [0, 0x80, 2, 255];
const DOTS: &str = "#.#.#.#.#.#.#.#. .#.#.#.#.#.#.#.#";
const LIT: &str = "################ ################";
const KEYS: &str = "0123456789%K";
const DIGITS: [&str; 12] = [
    "####### #.....# #.....# #.....# #.....# #.....# #.....# #.....# #.....# #.....# #######",
    ".....#. .....#. .....#. .....#. .....#. .....#. .....#. .....#. .....#. .....#. .....#.",
    "####### ......# ......# ......# ......# ####### #...... #...... #...... #...... #######",
    "####### ......# ......# ......# ......# ..##### ......# ......# ......# ......# #######",
    "#.....# #.....# #.....# #.....# #.....# ####### ......# ......# ......# ......# ......#",
    "####### #...... #...... #...... #...... ####### ......# ......# ......# ......# #######",
    "####### #...... #...... #...... #...... ####### #.....# #.....# #.....# #.....# #######",
    "####### ......# ......# ......# ......# ......# ......# ......# ......# ......# ......#",
    "####### #.....# #.....# #.....# #.....# ####### #.....# #.....# #.....# #.....# #######",
    "####### #.....# #.....# #.....# #.....# ####### ......# ......# ......# ......# #######",
    "....... .##..#. #..##.. .##.#.. ...#... ...#... ..#.... ..#.##. .#.#..# .#..##. .......",
    "....... #...#.. #..#... #.#.... ###.... #..#... #..#... #...#.. #...#.. #....#. .......",
];

pub(crate) struct Readings {
    pub(crate) cpu: Vec<u8>,
    pub(crate) kernel: Option<Vec<u8>>,
    pub(crate) memory: Vec<u8>,
    pub(crate) usage: u8,
    pub(crate) committed: u64,
    pub(crate) limit: u64,
    pub(crate) midnight: i32,
    pub(crate) blocks: [(&'static str, [(&'static str, u64); 3]); 4],
}

pub(crate) fn draw(canvas: &mut Canvas, panel: Rect, readings: &Readings) {
    let area = panel.shrink(16, 10, 16, 16);
    let lower = area.bottom() - 2 * BLOCK - 8;
    let row = (lower - 7 - area.y - 4) / 2;
    let usage = i32::from(readings.usage);
    let (committed, limit) = (readings.committed, readings.limit.max(1));
    let processor = Gauge {
        text: format!("{usage}%"),
        right: 48,
        inset: 0,
        lift: 0,
        lit: &|segments| (usage * segments + 99) / 100,
    };
    let memory = Gauge {
        text: format!("{committed}K"),
        right: 62,
        inset: 1,
        lift: 1,
        lit: &|segments| {
            let segments = u64::try_from(segments).unwrap_or_default();
            i32::try_from((committed * segments + limit / 2) / limit).unwrap_or_default()
        },
    };
    let mut usage: Vec<(&[u8], Color)> = vec![(&readings.cpu, GREEN)];
    if let Some(kernel) = &readings.kernel {
        usage.push((kernel, RED));
    }
    let first = Rect::new(area.x, area.y, area.width, row);
    chart(
        canvas,
        first,
        TITLES[0],
        &processor,
        &usage,
        readings.midnight,
    );
    let second = first.offset(0, row + 4);
    let memory_line: &[(&[u8], Color)] = &[(&readings.memory, YELLOW)];
    chart(
        canvas,
        second,
        TITLES[1],
        &memory,
        memory_line,
        readings.midnight,
    );
    let half = (area.width - 13) / 2;
    for (index, (title, rows)) in (0..).zip(&readings.blocks) {
        let (column, line) = (index % 2, index / 2);
        let x = area.x + column * (half + 13);
        let width = if column == 0 {
            half
        } else {
            area.width - half - 13
        };
        let block = Rect::new(x, lower + line * (BLOCK + 8), width, BLOCK);
        group(canvas, block, title);
        for (offset, (label, value)) in LINES[usize::from(line > 0)].into_iter().zip(rows) {
            let top = block.y + offset;
            FONT.draw(canvas, label, block.x + 9, top, BLACK);
            let value = value.to_string();
            FONT.draw(
                canvas,
                &value,
                block.right() - 11 - FONT.width(&value),
                top,
                BLACK,
            );
        }
    }
}

struct Gauge<'a> {
    text: String,
    right: i32,
    inset: i32,
    lift: i32,
    lit: &'a dyn Fn(i32) -> i32,
}

fn chart(
    canvas: &mut Canvas,
    area: Rect,
    titles: (&str, &str),
    gauge: &Gauge<'_>,
    lines: &[(&[u8], Color)],
    midnight: i32,
) {
    let meter = Rect::new(area.x, area.y, METER, area.height);
    group(canvas, meter, titles.0);
    let dial = Rect::new(
        meter.x + 8 + gauge.inset,
        meter.y + 20,
        72,
        area.height - 28,
    );
    draw_gauge(canvas, dial, gauge);
    let graph = Rect::sides(area.x + METER + GAP, area.y, area.right(), area.bottom());
    group(canvas, graph, titles.1);
    plot(
        canvas,
        Rect::new(
            graph.x + 8,
            graph.y + 20,
            graph.width - 16,
            area.height - 28,
        ),
        lines,
        midnight,
    );
}

fn group(canvas: &mut Canvas, area: Rect, title: &str) {
    controls::bevel(canvas, area.shrink(0, 6, 0, 0), ETCHED);
    let width = FONT.width(title);
    canvas.fill(
        Rect::new(area.x + 7, area.y, width + 4, FONT.height()),
        FACE,
    );
    FONT.draw(canvas, title, area.x + 9, area.y, BLACK);
}

fn draw_gauge(canvas: &mut Canvas, area: Rect, gauge: &Gauge<'_>) {
    controls::bevel(canvas, area, SUNKEN);
    let inside = area.inset(2);
    canvas.fill(inside, BLACK);
    let digits = inside.bottom() - 17;
    let bottom = digits - 6;
    let segments = (bottom - inside.y - 5) / 3;
    let lit = (gauge.lit)(segments);
    for segment in 0..segments {
        let y = bottom - 2 - 3 * segment;
        let (glyph, color, y) = if segment < lit {
            (LIT, GREEN, y - gauge.lift)
        } else {
            (DOTS, DIM, y)
        };
        for bar in [inside.x + 17, inside.x + 34] {
            canvas.stamp(bar, y, glyph, &[('#', color)]);
        }
    }
    let mut x = inside.x + gauge.right - 8 * i32::try_from(gauge.text.len()).unwrap_or_default();
    for character in gauge.text.chars() {
        if let Some(index) = KEYS.find(character) {
            canvas.stamp(x, digits, DIGITS[index], &[('#', GREEN)]);
        }
        x += 8;
    }
}

fn plot(canvas: &mut Canvas, area: Rect, lines: &[(&[u8], Color)], midnight: i32) {
    controls::bevel(canvas, area, SUNKEN);
    let inside = area.inset(2);
    canvas.fill(inside, BLACK);
    for y in (inside.y..inside.bottom() - 10).rev().step_by(GRID_STEP) {
        canvas.fill(Rect::new(inside.x, y, inside.width, 1), GRID);
    }
    for x in (inside.x..inside.right() - 1 - 2 * midnight)
        .rev()
        .step_by(GRID_STEP)
    {
        canvas.fill(Rect::new(x, inside.y, 1, inside.height), GRID);
    }
    canvas.clipped(inside, |canvas| {
        for &(samples, color) in lines {
            let points: Vec<(i32, i32)> = (0..)
                .zip(samples.iter().rev())
                .map(|(age, &value)| {
                    let rise = (i32::from(value) * inside.height + 50) / 100;
                    (inside.right() - SAMPLE * age, inside.bottom() - rise)
                })
                .collect();
            for pair in points.windows(2) {
                line(canvas, pair[0], pair[1], color);
            }
        }
    });
}

fn line(canvas: &mut Canvas, (x0, y0): (i32, i32), (x1, y1): (i32, i32), color: Color) {
    let steps = (x1 - x0).abs().max((y1 - y0).abs());
    let along = |delta: i32, step: i32| -((steps - 2 * delta * step).div_euclid(2 * steps));
    for step in 0..steps {
        canvas.plot(x0 + along(x1 - x0, step), y0 + along(y1 - y0, step), color);
    }
}
