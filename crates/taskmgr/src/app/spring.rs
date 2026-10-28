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

use jiff::{SignedDuration, Timestamp};

use crate::app::{App, Place};
use crate::graphics::Canvas;
use crate::ui::Target;

const SPRING: i64 = 500;

#[derive(Debug)]
pub(super) struct Spring {
    home: Vec<usize>,
    pub(super) source: Place,
    pub(super) head: usize,
    pub(super) depth: usize,
    pub(super) sketch: (Canvas, i32),
}

impl App {
    pub(super) fn linger(&mut self) {
        let over = match (self.pressed, self.hit()) {
            (Some(Target::Row(_)), Some(Target::Tab(depth, index))) if self.moved => {
                Some((depth, index))
            }
            _ => None,
        };
        if over != self.hovering.map(|(depth, index, _)| (depth, index)) {
            self.hovering = over.map(|(depth, index)| (depth, index, self.now.timestamp()));
        }
        self.unfold();
    }

    pub(super) fn ripe(&self) -> Option<Timestamp> {
        self.hovering
            .map(|(.., since)| since + SignedDuration::from_millis(SPRING))
    }

    fn unfold(&mut self) {
        let now = self.now.timestamp();
        let Some((depth, index, since)) = self.hovering else {
            return;
        };
        if self.ripe().is_some_and(|ripe| now < ripe && now >= since) {
            return;
        }
        self.hovering = None;
        let Some(place) = self.tab_place(depth, index) else {
            return;
        };
        if Some(&place) == self.place().as_ref() || !self.hauling() {
            return;
        }
        if self.spring.is_none() {
            let Some(Target::Row(key)) = self.pressed else {
                return;
            };
            let (Some(source), Some((_, head, depth)), Some(sketch)) =
                (self.place(), self.cargo(), self.sketch(key))
            else {
                return;
            };
            self.spring = Some(Spring {
                home: self.state.current().path(),
                source,
                head,
                depth,
                sketch,
            });
        }
        self.select(depth, index);
    }

    pub(super) fn abroad(&self) -> bool {
        self.spring
            .as_ref()
            .is_some_and(|spring| Some(&spring.source) != self.place().as_ref())
    }

    pub(super) fn source(&self) -> Option<Place> {
        self.spring
            .as_ref()
            .map(|spring| spring.source.clone())
            .or_else(|| self.place())
    }

    pub(super) fn home(&mut self) {
        self.hovering = None;
        if let Some(spring) = self.spring.take() {
            self.go(&spring.home);
        }
    }
}
