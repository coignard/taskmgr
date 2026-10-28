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

pub(crate) mod tracking;

use crate::command::Command;
use crate::controls::{self, Ink, RAISED};
use crate::geometry::{Rect, Size};
use crate::graphics::{BLACK, FACE, HIGHLIGHT, SHADOW, WHITE};
use crate::ui::{Target, Ui};

const BORDER: i32 = 3;
const ITEM: i32 = 17;
const SEPARATOR: i32 = 9;
const GUTTER: i32 = 17;
const MARGIN: i32 = 21;
const TAB: i32 = 12;
const CHECK: &str = "......# .....## #...### ##.###. #####.. .###... ..#....";
const BULLET: &str = ".####. ###### ###### ###### ###### .####.";
const ARROW: &str = "#... ##.. ###. #### ###. ##.. #...";

pub(crate) enum Entry {
    Item(String, Command),
    Submenu(String, Vec<Entry>),
    Separator,
}

pub(crate) struct Menu {
    pub(crate) title: &'static str,
    pub(crate) entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Clear,
    Check,
    Bullet,
}

impl Entry {
    pub(crate) fn label(&self) -> &str {
        self.columns().0
    }

    fn columns(&self) -> (&str, &str) {
        match self {
            Self::Item(text, _) | Self::Submenu(text, _) => {
                text.split_once('\t').unwrap_or((text, ""))
            }
            Self::Separator => ("", ""),
        }
    }

    fn height(&self) -> i32 {
        match self {
            Self::Separator => SEPARATOR,
            _ => ITEM,
        }
    }

    pub(crate) fn help(&self) -> &'static str {
        match self {
            Self::Item(_, command) => command.help(),
            _ => "",
        }
    }
}

fn widest(entries: &[Entry]) -> (i32, i32) {
    entries.iter().fold((0, 0), |(label, keys), entry| {
        let (left, right) = entry.columns();
        (
            label.max(controls::width(left)),
            keys.max(controls::width(right)),
        )
    })
}

pub(crate) fn size(entries: &[Entry]) -> Size {
    let (label, keys) = widest(entries);
    let keys = if keys > 0 { TAB + keys } else { 0 };
    let height = entries.iter().map(Entry::height).sum::<i32>();
    Size::new(
        BORDER + GUTTER + label + keys + MARGIN + BORDER,
        BORDER + height + BORDER,
    )
}

pub(crate) fn rows(entries: &[Entry]) -> impl Iterator<Item = Rect> + '_ {
    let width = size(entries).width - 2 * BORDER;
    entries.iter().scan(BORDER, move |top, entry| {
        let row = Rect::new(BORDER, *top, width, entry.height());
        *top += row.height;
        Some(row)
    })
}

pub(crate) fn submenu(entries: &[Entry], frame: Rect, index: usize) -> Rect {
    let row = rows(entries).nth(index).unwrap_or_default();
    let size = match entries.get(index) {
        Some(Entry::Submenu(_, inner)) => self::size(inner),
        _ => Size::default(),
    };
    Rect::new(
        frame.x + row.right() - BORDER,
        frame.y + row.y - BORDER,
        size.width,
        size.height,
    )
}

pub(crate) fn draw(
    ui: &mut Ui,
    level: usize,
    entries: &[Entry],
    highlighted: Option<usize>,
    underline: bool,
    look: impl Fn(Command) -> (Mark, bool),
) {
    let area = Rect::new(0, 0, ui.canvas.size().width, ui.canvas.size().height);
    ui.target(area, Target::Popup);
    controls::bevel(&mut ui.canvas, area, RAISED);
    let tab = GUTTER + widest(entries).0 + TAB;
    for (index, (entry, row)) in entries.iter().zip(rows(entries)).enumerate() {
        let lit = highlighted == Some(index);
        let (mark, enabled) = match entry {
            Entry::Item(_, command) => look(*command),
            Entry::Submenu(..) => (Mark::Clear, true),
            Entry::Separator => {
                let line = Rect::new(row.x + 1, row.y + 3, row.width - 2, 1);
                ui.canvas.fill(line, SHADOW);
                ui.canvas.fill(line.offset(0, 1), WHITE);
                continue;
            }
        };
        ui.target(row, Target::Item(level, index));
        ui.canvas.fill(row, if lit { HIGHLIGHT } else { FACE });
        let ink = match (lit, enabled) {
            (true, true) => Ink::Plain(WHITE),
            (true, false) => Ink::Plain(SHADOW),
            (false, true) => Ink::Plain(BLACK),
            (false, false) => Ink::Embossed,
        };
        let (label, keys) = entry.columns();
        controls::label(
            &mut ui.canvas,
            label,
            row.x + GUTTER,
            row.y + 1,
            ink,
            underline,
        );
        controls::label(&mut ui.canvas, keys, row.x + tab, row.y + 1, ink, false);
        let color = if lit { WHITE } else { BLACK };
        match mark {
            Mark::Check => ui
                .canvas
                .stamp(row.x + 4, row.y + 5, CHECK, &[('#', color)]),
            Mark::Bullet => ui
                .canvas
                .stamp(row.x + 5, row.y + 5, BULLET, &[('#', color)]),
            Mark::Clear => {}
        }
        if let Entry::Submenu(..) = entry {
            ui.canvas
                .stamp(row.right() - 10, row.y + 4, ARROW, &[('#', color)]);
        }
    }
}
