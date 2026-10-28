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

use std::cmp::Ordering;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assets::FONT;
use crate::controls::{self, BUTTON, FLAT, SUNKEN};
use crate::geometry::Rect;
use crate::graphics::{BLACK, Canvas, Color, FACE, HIGHLIGHT, Image, SHADOW, TRANSPARENT, WHITE};
use crate::scroll::{self, Bar};
use crate::ui::{Target, Ui};

const HEADER: i32 = 17;
const LINE: i32 = 13;
pub(crate) const ROW: i32 = LINE + 1;
const BADGE: i32 = 14;
const LEAD: i32 = 2;
const MARGIN: i32 = 2;
const PADDING: i32 = 6;
const LABEL: i32 = 4;
const DIVIDER: i32 = 4;
const NUDGE: i32 = 8;
pub(crate) const INDENT: i32 = 16;

#[derive(Clone, Debug)]
pub(crate) struct Column {
    pub(crate) title: &'static str,
    pub(crate) width: i32,
    pub(crate) numeric: bool,
}

#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) key: u32,
    pub(crate) level: usize,
    pub(crate) icon: Option<&'static Image>,
    pub(crate) cells: Vec<String>,
}

impl Row {
    pub(crate) fn badge(&self) -> i32 {
        BADGE * i32::from(self.icon.is_some())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Mark {
    pub(crate) index: usize,
    pub(crate) level: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct List {
    pub(crate) widths: Vec<i32>,
    pub(crate) sorted: Option<(usize, bool)>,
    #[serde(alias = "selected")]
    pub(crate) focus: Option<u32>,
    pub(crate) selection: BTreeSet<u32>,
    pub(crate) scrolled: [i32; 2],
    #[serde(skip)]
    pub(crate) anchor: Option<u32>,
    #[serde(skip)]
    pub(crate) columns: Vec<Column>,
    #[serde(skip)]
    pub(crate) rows: Vec<Row>,
}

pub(crate) struct Layout {
    pub(crate) body: Rect,
    pub(crate) bars: [Option<Bar>; 2],
}

impl List {
    pub(crate) fn fill(&mut self, columns: Vec<Column>, rows: Vec<Row>) {
        self.widths.truncate(columns.len());
        let known = self.widths.len();
        self.widths
            .extend(columns[known..].iter().map(|column| column.width));
        let sorted = self.sorted.filter(|&(column, _)| column < columns.len());
        self.rows = match sorted {
            Some((column, ascending)) => arrange(rows, column, columns[column].numeric, ascending),
            None => rows,
        };
        self.columns = columns;
        let keys: BTreeSet<u32> = self.rows.iter().map(|row| row.key).collect();
        self.selection.retain(|key| keys.contains(key));
        for key in [&mut self.focus, &mut self.anchor] {
            *key = key.filter(|key| keys.contains(key));
        }
    }

    pub(crate) fn sort(&mut self, column: usize) {
        let ascending = self.sorted != Some((column, true));
        self.sorted = Some((column, ascending));
    }

    pub(crate) fn index(&self) -> Option<usize> {
        self.position(self.focus?)
    }

    pub(crate) fn position(&self, key: u32) -> Option<usize> {
        self.rows.iter().position(|row| row.key == key)
    }

    pub(crate) fn selected(&self) -> Vec<u32> {
        self.rows
            .iter()
            .map(|row| row.key)
            .filter(|key| self.selection.contains(key))
            .collect()
    }

    pub(crate) fn choose(&mut self, keys: impl IntoIterator<Item = u32>) {
        self.selection = keys.into_iter().collect();
        self.focus = self
            .rows
            .iter()
            .map(|row| row.key)
            .find(|key| self.selection.contains(key));
        self.focus = self.focus.or_else(|| self.selection.first().copied());
        self.anchor = self.focus;
    }

    pub(crate) fn click(&mut self, key: u32, toggle: bool, extend: bool) {
        match (toggle, extend) {
            (_, true) => self.reach(key, toggle),
            (true, false) => {
                if !self.selection.remove(&key) {
                    self.selection.insert(key);
                }
                self.focus = Some(key);
                self.anchor = Some(key);
            }
            (false, false) => self.choose([key]),
        }
    }

    pub(crate) fn go(&mut self, index: usize, extend: bool) {
        let Some(row) = self.rows.get(index.min(self.rows.len().saturating_sub(1))) else {
            return;
        };
        let key = row.key;
        if extend {
            self.reach(key, false);
        } else {
            self.choose([key]);
        }
    }

    fn reach(&mut self, key: u32, keep: bool) {
        let anchor = self.anchor.or(self.focus).unwrap_or(key);
        let (Some(from), Some(to)) = (self.position(anchor), self.position(key)) else {
            return self.choose([key]);
        };
        if !keep {
            self.selection.clear();
        }
        let span = from.min(to)..=from.max(to);
        self.selection
            .extend(self.rows[span].iter().map(|row| row.key));
        self.anchor = Some(anchor);
        self.focus = Some(key);
    }

    pub(crate) fn start(&self, column: usize) -> i32 {
        self.widths[..column.min(self.widths.len())].iter().sum()
    }

    fn width(&self) -> i32 {
        self.start(self.widths.len())
    }

    pub(crate) fn layout(&self, inside: Rect) -> Layout {
        let rows = i32::try_from(self.rows.len()).unwrap_or(i32::MAX);
        let width = self.width();
        let (lead, height) = (LEAD, ROW);
        let tall = |across: bool| {
            lead + rows * height > inside.height - HEADER - scroll::BREADTH * i32::from(across)
        };
        let wide = |down: bool| width > inside.width - scroll::BREADTH * i32::from(down);
        let down = tall(wide(tall(false)));
        let across = wide(down);
        let body = inside.shrink(
            0,
            HEADER,
            scroll::BREADTH * i32::from(down),
            scroll::BREADTH * i32::from(across),
        );
        let page = ((body.height - lead) / height).max(1);
        let vertical = Bar {
            area: Rect::sides(body.right(), inside.y, inside.right(), body.bottom()),
            vertical: true,
            position: self.scrolled[1].clamp(0, (rows - page).max(0)),
            page,
            total: rows,
        };
        let horizontal = Bar {
            area: Rect::sides(inside.x, body.bottom(), body.right(), inside.bottom()),
            vertical: false,
            position: self.scrolled[0].clamp(0, (width - body.width).max(0)),
            page: body.width,
            total: width,
        };
        Layout {
            body,
            bars: [across.then_some(horizontal), down.then_some(vertical)],
        }
    }

    pub(crate) fn scroll(&mut self, inside: Rect, vertical: bool, to: impl FnOnce(&Bar) -> i32) {
        let Some(bar) = self.layout(inside).bars[usize::from(vertical)] else {
            return;
        };
        self.scrolled[usize::from(vertical)] = to(&bar).clamp(0, (bar.total - bar.page).max(0));
    }

    pub(crate) fn nudge(&mut self, inside: Rect, vertical: bool, steps: i32) {
        let unit = if vertical { 1 } else { NUDGE };
        self.scroll(inside, vertical, |bar| bar.position + steps * unit);
    }

    pub(crate) fn turn(&mut self, inside: Rect, vertical: bool, pages: i32) {
        self.scroll(inside, vertical, |bar| bar.position + pages * bar.page);
    }

    pub(crate) fn landing(&self, inside: Rect, y: i32) -> usize {
        let Layout { body, bars } = self.layout(inside);
        let down = bars[1].map_or(0, |bar| bar.position);
        let (lead, height) = (LEAD, ROW);
        let row = (y - body.y - lead + height / 2).div_euclid(height) + down;
        usize::try_from(row.max(0))
            .unwrap_or(0)
            .min(self.rows.len())
    }

    pub(crate) fn reveal(&mut self, inside: Rect) {
        let Some(row) = self.index().and_then(|row| i32::try_from(row).ok()) else {
            return;
        };
        self.scroll(inside, true, |bar| {
            bar.position.clamp(row - bar.page + 1, row)
        });
    }
}

pub(crate) fn page(inside: Rect) -> usize {
    usize::try_from(((inside.height - HEADER - LEAD) / ROW).max(1)).unwrap_or(1)
}

fn arrange(rows: Vec<Row>, column: usize, numeric: bool, ascending: bool) -> Vec<Row> {
    let mut stack: Vec<usize> = Vec::new();
    let mut paths: Vec<(Vec<usize>, usize)> = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        stack.truncate(row.level);
        stack.push(index);
        paths.push((stack.clone(), index));
    }
    let order = |a: usize, b: usize| {
        let order = compare(&rows[a].cells[column], &rows[b].cells[column], numeric);
        let order = if ascending { order } else { order.reverse() };
        order.then(a.cmp(&b))
    };
    paths.sort_by(|(a, _), (b, _)| {
        a.iter()
            .zip(b)
            .map(|(&a, &b)| order(a, b))
            .find(|order| order.is_ne())
            .unwrap_or_else(|| a.len().cmp(&b.len()))
    });
    let mut rows: Vec<Option<Row>> = rows.into_iter().map(Some).collect();
    paths
        .into_iter()
        .filter_map(|(_, index)| rows[index].take())
        .collect()
}

fn compare(a: &str, b: &str, numeric: bool) -> Ordering {
    let number = |text: &str| {
        let digits: String = text.chars().filter(char::is_ascii_digit).collect();
        digits.parse::<u64>().ok()
    };
    if numeric {
        number(a).cmp(&number(b))
    } else {
        a.to_lowercase().cmp(&b.to_lowercase())
    }
}

pub(crate) fn draw(ui: &mut Ui, area: Rect, list: &List, focused: bool, mark: Option<Mark>) {
    controls::bevel(&mut ui.canvas, area, SUNKEN);
    let inside = area.inset(2);
    ui.canvas.fill(inside, WHITE);
    ui.target(inside, Target::List);
    let Layout { body, bars } = list.layout(inside);
    let [across, down] = bars.map(|bar| bar.map_or(0, |bar| bar.position));
    let origin = body.x - across;
    header(
        ui,
        Rect::sides(inside.x, inside.y, body.right(), body.y),
        origin,
        list,
    );
    let (lead, height) = (LEAD, ROW);
    let width = list.width();
    let lines = branches(&list.rows);
    for (index, row) in (0..).zip(&list.rows) {
        if index < down {
            continue;
        }
        let top = body.y + lead + height * (index - down);
        if top >= body.bottom() {
            break;
        }
        let selected = list.selection.contains(&row.key);
        let (fill, ink) = match (selected, focused || mark.is_some()) {
            (true, true) => (Some(HIGHLIGHT), WHITE),
            (true, false) => (Some(FACE), BLACK),
            _ => (None, BLACK),
        };
        let outline = list.focus == Some(row.key) && focused && ui.cues.focus;
        let indent = INDENT * i32::try_from(row.level).unwrap_or_default();
        let badge = row.badge();
        ui.canvas.clipped(body, |canvas| {
            let mut left = origin;
            let specs = list.columns.iter().zip(&list.widths);
            for (column, ((spec, &span), text)) in specs.zip(&row.cells).enumerate() {
                let cell = Rect::new(left, top, span, height);
                left = cell.right();
                if column > 0 {
                    write(canvas, cell, body, text, spec.numeric, PADDING, ink);
                    continue;
                }
                let cell = cell.shrink(indent, 0, 0, 0);
                branch(
                    canvas,
                    cell,
                    &lines[usize::try_from(index).unwrap_or_default()],
                );
                if let Some(icon) = row.icon {
                    ball(
                        canvas,
                        icon,
                        cell.x + MARGIN + 1,
                        top + 2,
                        fill == Some(HIGHLIGHT),
                    );
                }
                let band = Rect::new(
                    cell.x + MARGIN + badge,
                    top,
                    width - MARGIN - indent - badge,
                    height,
                );
                if let Some(fill) = fill {
                    canvas.fill(band, fill);
                }
                write(
                    canvas,
                    cell.shrink(badge, 0, 0, 0),
                    body,
                    text,
                    false,
                    LABEL,
                    ink,
                );
                if outline {
                    controls::focus(canvas, band, WHITE);
                }
            }
        });
        let row_area = Rect::new(origin, top, width, height).intersect(body);
        ui.target(row_area, Target::Row(row.key));
    }
    if let Some(mark) = mark {
        let index = i32::try_from(mark.index).unwrap_or_default();
        let y = body.y + lead + height * (index - down) - 1;
        let indent = INDENT * i32::try_from(mark.level).unwrap_or_default();
        let line = Rect::sides(origin + MARGIN + indent, y - 1, origin + width, y + 1);
        ui.canvas.clipped(body, |canvas| insertion(canvas, line));
    }
    scrollbars(ui, bars);
}

fn scrollbars(ui: &mut Ui, bars: [Option<Bar>; 2]) {
    for bar in bars.into_iter().flatten() {
        bar.draw(ui);
    }
    if let [Some(across), Some(down)] = bars {
        let corner = Rect::sides(
            down.area.x,
            across.area.y,
            down.area.right(),
            across.area.bottom(),
        );
        ui.canvas.fill(corner, FACE);
    }
}

fn ball(canvas: &mut Canvas, icon: &Image, x: i32, y: i32, selected: bool) {
    canvas.draw(icon, x, y);
    if !selected {
        return;
    }
    let size = icon.size();
    for down in 0..size.height {
        for across in 0..size.width {
            if (x + across + y + down) % 2 == 0 && icon.pixel(across, down) != TRANSPARENT {
                canvas.plot(x + across, y + down, HIGHLIGHT);
            }
        }
    }
}

fn insertion(canvas: &mut Canvas, line: Rect) {
    canvas.fill(line, BLACK);
    for x in [line.x, line.right() - 2] {
        canvas.fill(Rect::new(x, line.y - 2, 2, 6), BLACK);
    }
}

#[derive(Debug, Default)]
struct Branch {
    level: usize,
    through: Vec<bool>,
}

fn branches(rows: &[Row]) -> Vec<Branch> {
    let mut below: Vec<bool> = Vec::new();
    let mut lines: Vec<Branch> = rows
        .iter()
        .rev()
        .map(|row| {
            below.resize(row.level + 1, false);
            let through = below.clone();
            below[row.level] = true;
            Branch {
                level: row.level,
                through,
            }
        })
        .collect();
    lines.reverse();
    lines
}

fn branch(canvas: &mut Canvas, cell: Rect, branch: &Branch) {
    if branch.level == 0 {
        return;
    }
    let middle = cell.y + cell.height / 2;
    let mut dotted = |area: Rect| {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if (x + y) % 2 == 0 {
                    canvas.plot(x, y, SHADOW);
                }
            }
        }
    };
    for depth in 1..=branch.level {
        let back = INDENT * i32::try_from(branch.level - depth + 1).unwrap_or_default();
        let x = cell.x + LABEL + 4 - back;
        let continues = branch.through.get(depth).copied().unwrap_or(false);
        if depth == branch.level {
            let bottom = if continues { cell.bottom() } else { middle + 1 };
            dotted(Rect::sides(x, cell.y, x + 1, bottom));
            dotted(Rect::sides(x, middle, cell.x + LABEL - 2, middle + 1));
        } else if continues {
            dotted(Rect::sides(x, cell.y, x + 1, cell.bottom()));
        }
    }
}

fn header(ui: &mut Ui, strip: Rect, origin: i32, list: &List) {
    let mut left = origin;
    for (index, (column, &width)) in list.columns.iter().zip(&list.widths).enumerate() {
        let item = Rect::new(left, strip.y, width, HEADER);
        left = item.right();
        let pushed = ui.held(Target::Column(index));
        ui.canvas.clipped(strip, |canvas| {
            controls::knob(canvas, item, if pushed { FLAT } else { BUTTON });
            let shift = i32::from(pushed);
            let text = item.offset(shift, shift);
            write(
                canvas,
                text,
                item.inset(1),
                column.title,
                column.numeric,
                PADDING,
                BLACK,
            );
        });
        ui.target(item.intersect(strip), Target::Column(index));
    }
    let mut edge = origin;
    for (index, &width) in list.widths.iter().enumerate() {
        edge += width;
        let divider = Rect::new(edge - DIVIDER, strip.y, 2 * DIVIDER, HEADER);
        ui.target(divider.intersect(strip), Target::Divider(index));
    }
    ui.canvas.clipped(strip, |canvas| {
        let filler = Rect::sides(left, strip.y, strip.right() + 2, strip.bottom());
        canvas.fill(filler, FACE);
        controls::bevel(canvas, filler, BUTTON);
    });
}

fn write(
    canvas: &mut Canvas,
    cell: Rect,
    bounds: Rect,
    text: &str,
    right: bool,
    padding: i32,
    ink: Color,
) {
    let lane = Rect::sides(
        cell.x + padding,
        bounds.y,
        cell.right() - padding,
        bounds.bottom(),
    );
    canvas.clipped(lane, |canvas| {
        let text = FONT.ellipsized(text, lane.width);
        let x = if right {
            lane.right() - FONT.width(&text)
        } else {
            lane.x
        };
        FONT.draw(canvas, &text, x, cell.y + (cell.height - LINE) / 2, ink);
    });
}
