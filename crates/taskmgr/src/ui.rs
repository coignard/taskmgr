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

use crate::command::Command;
use crate::dialog;
use crate::geometry::{Point, Rect, Size};
use crate::graphics::{Canvas, FACE};
use crate::scroll::Part;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Frame,
    Caption,
    Minimize,
    Zoom,
    Close,
    Title(usize),
    Item(usize, usize),
    Popup,
    Tab(usize, usize),
    Spin(usize, bool),
    Button(Command),
    Column(usize),
    Divider(usize),
    Row(u32),
    List,
    Scroll(bool, Part),
    Dialog(dialog::Part),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cues {
    pub(crate) keys: bool,
    pub(crate) focus: bool,
}

pub(crate) struct Ui {
    pub(crate) canvas: Canvas,
    pub(crate) origin: Point,
    pub(crate) targets: Vec<(Rect, Target)>,
    pub(crate) held: Option<Target>,
    pub(crate) cues: Cues,
}

impl Ui {
    pub(crate) fn new(size: Size, held: Option<Target>, cues: Cues) -> Self {
        Self {
            canvas: Canvas::new(size, FACE),
            origin: Point::default(),
            targets: Vec::new(),
            held,
            cues,
        }
    }

    pub(crate) fn target(&mut self, area: Rect, target: Target) {
        let area = area.offset(self.origin.x, self.origin.y);
        self.targets.push((area, target));
    }

    pub(crate) fn held(&self, target: Target) -> bool {
        self.held == Some(target)
    }
}

pub(crate) fn hit(targets: &[(Rect, Target)], point: Point) -> Option<Target> {
    targets
        .iter()
        .rev()
        .find(|(area, _)| area.contains(point))
        .map(|&(_, target)| target)
}
