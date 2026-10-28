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

use std::mem;

use jiff::{SignedDuration, Timestamp};

use crate::chrome;
use crate::command::Command;
use crate::controls;
use crate::geometry::Rect;
use crate::keys::Key;
use crate::menu::{self, Entry, Menu};
use crate::ui::Target;

const DELAY: SignedDuration = SignedDuration::from_millis(400);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Stay,
    Close,
    Run(Command),
}

pub(crate) struct Tracking {
    menus: Vec<Menu>,
    pub(crate) title: usize,
    highlights: Vec<Option<usize>>,
    pub(crate) due: Option<Timestamp>,
    pub(crate) keyboard: bool,
    pub(crate) help: &'static str,
}

impl Tracking {
    pub(crate) fn new(menus: Vec<Menu>, title: usize, dropped: bool, keyboard: bool) -> Self {
        let mut tracking = Self {
            menus,
            title,
            highlights: if dropped { vec![None] } else { Vec::new() },
            due: None,
            keyboard,
            help: "",
        };
        if dropped && keyboard {
            tracking.step(1);
        }
        tracking
    }

    pub(crate) fn dropped(&self) -> bool {
        !self.highlights.is_empty()
    }

    pub(crate) fn chain(&self) -> Vec<(Rect, &[Entry], Option<usize>)> {
        let Some(title) = chrome::titles(&self.menus).nth(self.title) else {
            return Vec::new();
        };
        let mut entries = &self.menus[self.title].entries[..];
        let mut frame = Rect::new(title.x, title.bottom(), 0, 0);
        let mut chain = Vec::new();
        for &highlight in &self.highlights {
            let size = menu::size(entries);
            frame = Rect::new(frame.x, frame.y, size.width, size.height);
            chain.push((frame, entries, highlight));
            if let Some(index) = highlight
                && let Some(Entry::Submenu(_, inner)) = entries.get(index)
            {
                frame = menu::submenu(entries, frame, index);
                entries = inner;
            }
        }
        chain
    }

    pub(crate) fn tick(&mut self, now: Timestamp) {
        if self.due.is_some_and(|due| now >= due) {
            self.expand(false);
        }
    }

    pub(crate) fn hover(&mut self, target: Option<Target>, now: Timestamp) {
        if !self.dropped() {
            return;
        }
        match target {
            Some(Target::Title(title)) if title != self.title => self.open(title, true, false),
            Some(Target::Item(level, index)) => {
                self.highlight(level, Some(index), Some(now + DELAY));
            }
            Some(Target::Popup) => {}
            _ => {
                let level = self.highlights.len() - 1;
                if self.due.is_some() || !self.submenu(level, self.highlights[level]) {
                    self.highlight(level, None, None);
                }
            }
        }
    }

    pub(crate) fn press(&mut self, target: Option<Target>) -> Outcome {
        match target {
            Some(Target::Title(title)) if title == self.title => Outcome::Close,
            Some(Target::Title(title)) => {
                self.open(title, true, false);
                Outcome::Stay
            }
            Some(Target::Item(level, index)) => {
                self.highlight(level, Some(index), None);
                if self.submenu(level, Some(index)) {
                    self.expand(false);
                }
                Outcome::Stay
            }
            Some(Target::Popup) => Outcome::Stay,
            _ => Outcome::Close,
        }
    }

    pub(crate) fn release(&mut self, target: Option<Target>) -> Outcome {
        match target {
            Some(Target::Item(level, index)) => self.activate(level, index),
            _ => Outcome::Stay,
        }
    }

    pub(crate) fn key(&mut self, key: Key) -> Outcome {
        self.keyboard = true;
        let depth = self.highlights.len();
        let level = depth.saturating_sub(1);
        let highlight = self.highlights.last().copied().flatten();
        let titles = self.menus.len();
        match key {
            Key::Escape | Key::Left if depth > 1 => self.pop(),
            Key::Escape if depth == 1 => self.highlights.clear(),
            Key::Escape => return Outcome::Close,
            Key::Right if self.submenu(level, highlight) => self.expand(true),
            Key::Left | Key::Right => {
                let step = if key == Key::Left { titles - 1 } else { 1 };
                self.open((self.title + step) % titles, depth > 0, true);
            }
            Key::Up | Key::Down | Key::Enter if depth == 0 => self.open(self.title, true, true),
            Key::Up => self.step(-1),
            Key::Down => self.step(1),
            Key::Enter => {
                return highlight.map_or(Outcome::Stay, |index| self.activate(level, index));
            }
            Key::Character(letter) if depth == 0 => {
                let found = self
                    .menus
                    .iter()
                    .position(|menu| controls::accelerator(menu.title) == Some(letter));
                if let Some(title) = found {
                    self.open(title, true, true);
                }
            }
            Key::Character(letter) => {
                let found = self
                    .entries(level)
                    .iter()
                    .position(|entry| controls::accelerator(entry.label()) == Some(letter));
                if let Some(index) = found {
                    self.highlight(level, Some(index), None);
                    return self.activate(level, index);
                }
            }
            _ => {}
        }
        Outcome::Stay
    }

    fn open(&mut self, title: usize, dropped: bool, keyboard: bool) {
        *self = Self::new(mem::take(&mut self.menus), title, dropped, keyboard);
    }

    fn entries(&self, level: usize) -> &[Entry] {
        let mut entries = self
            .menus
            .get(self.title)
            .map_or(&[][..], |menu| &menu.entries);
        for highlight in self.highlights.iter().take(level) {
            match highlight.and_then(|index| entries.get(index)) {
                Some(Entry::Submenu(_, inner)) => entries = inner,
                _ => return &[],
            }
        }
        entries
    }

    fn entry(&self, level: usize, index: Option<usize>) -> Option<&Entry> {
        self.entries(level).get(index?)
    }

    fn submenu(&self, level: usize, index: Option<usize>) -> bool {
        matches!(self.entry(level, index), Some(Entry::Submenu(..)))
    }

    fn highlight(&mut self, level: usize, index: Option<usize>, due: Option<Timestamp>) {
        let entry = self.entry(level, index);
        let submenu = matches!(entry, Some(Entry::Submenu(..)));
        if !submenu {
            self.help = entry.map_or("", Entry::help);
        }
        if self.highlights.get(level) == Some(&index) {
            if self.highlights.len() > level + 1 {
                self.highlights.truncate(level + 2);
                self.highlights[level + 1] = None;
            }
            return;
        }
        self.highlights.truncate(level + 1);
        self.highlights[level] = index;
        self.due = due.filter(|_| submenu);
    }

    fn expand(&mut self, first: bool) {
        self.due = None;
        self.highlights.push(None);
        if first {
            self.step(1);
        }
    }

    fn pop(&mut self) {
        self.highlights.pop();
        self.due = None;
    }

    fn step(&mut self, direction: isize) {
        let Some(level) = self.highlights.len().checked_sub(1) else {
            return;
        };
        let highlight = self.highlights[level];
        let entries = self.entries(level);
        let count = entries.len().cast_signed();
        let start = highlight.map_or(if direction > 0 { -1 } else { count }, usize::cast_signed);
        let next = (1..=count)
            .map(|offset| {
                (start + direction * offset)
                    .rem_euclid(count)
                    .cast_unsigned()
            })
            .find(|&index| !matches!(entries[index], Entry::Separator));
        self.highlight(level, next, None);
    }

    fn activate(&mut self, level: usize, index: usize) -> Outcome {
        match self.entry(level, Some(index)) {
            Some(Entry::Item(_, command)) => Outcome::Run(*command),
            Some(Entry::Submenu(..)) => {
                self.expand(self.keyboard);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}
