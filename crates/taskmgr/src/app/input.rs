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

use crate::app::{App, Effect};
use crate::chrome;
use crate::command::{Command, Kind};
use crate::controls;
use crate::dialog::{Outcome, Part};
use crate::geometry::{Point, Rect};
use crate::keys::{Key, Keystroke};
use crate::list;
use crate::menu::tracking::Tracking;
use crate::pages::Focus;
use crate::scroll::Part as Bit;
use crate::ui::Target;

const DRAG: i32 = 4;
const EDGE: i32 = 8;
const SWIPE: i32 = 24;

impl App {
    pub(super) fn hover(&mut self) {
        let target = self.hit();
        let now = self.now.timestamp();
        if let Some(menu) = &mut self.menu {
            menu.hover(target, now);
            return;
        }
        let (pointer, anchor, grab) = (self.pointer, self.anchor, self.grab);
        if let Some(pressed) = self.pressed
            && !self.moved
        {
            let slack = match pressed {
                Target::Caption | Target::Frame | Target::Dialog(Part::Caption) => 0,
                _ => DRAG - 1,
            };
            self.moved =
                (pointer.x - anchor.x).abs() > slack || (pointer.y - anchor.y).abs() > slack;
        }
        let held = self.pressed == Some(Target::Dialog(Part::Field));
        if let Some(sheet) = self.sheets.last_mut() {
            let part = match target {
                Some(Target::Dialog(part)) => Some(part),
                _ => None,
            };
            sheet.dialog.drag(part, pointer.x - sheet.at.x, held);
            return;
        }
        match self.pressed {
            Some(Target::Tab(depth, from)) if self.moved && !self.alt => self.shuffle(depth, from),
            Some(Target::Scroll(vertical, Bit::Thumb)) => {
                let Some(inside) = self.inside() else {
                    return;
                };
                let along = if vertical {
                    pointer.y - grab.y
                } else {
                    pointer.x - grab.x
                };
                if let Some(list) = self.list_mut() {
                    list.scroll(inside, vertical, |bar| bar.dragged(along));
                    self.dirty = true;
                }
            }
            _ => {}
        }
    }

    pub(super) fn press(&mut self, clicks: u8) -> Option<Effect> {
        let target = self.hit();
        let double = clicks >= 2 && clicks.is_multiple_of(2);
        self.alone = false;
        self.pressed = target;
        self.anchor = self.pointer;
        self.moved = false;
        self.deferred = None;
        if let Some(menu) = &mut self.menu {
            let outcome = menu.press(target);
            return self.follow(outcome);
        }
        if let Some(sheet) = self.sheets.last_mut() {
            let Some(Target::Dialog(part)) = target else {
                self.pressed = None;
                return Some(Effect::Beep);
            };
            sheet
                .dialog
                .press(part, self.pointer.x - sheet.at.x, clicks);
            self.blink = self.now.timestamp();
            return None;
        }
        match target? {
            Target::Caption if double => {
                self.pressed = None;
                Some(self.zoom())
            }
            Target::Title(title) => {
                self.menu = Some(Tracking::new(self.menus(), title, true, false));
                None
            }
            Target::Tab(depth, index) => {
                self.select(depth, index);
                self.focus = Focus::Tabs(depth);
                if double && let Some(path) = self.tab_project(depth, index) {
                    self.pressed = None;
                    self.rename_project(path);
                }
                None
            }
            Target::Spin(depth, forth) => {
                self.spin(depth, forth);
                None
            }
            Target::Row(key) => {
                self.focus = Focus::List;
                if let Some(&(row, _)) = self
                    .targets
                    .iter()
                    .find(|&&(_, target)| target == Target::Row(key))
                {
                    self.grab = Point::new(self.pointer.x - row.x, self.pointer.y - row.y);
                }
                let held = self.held;
                let list = self.list_mut()?;
                if double {
                    list.choose([key]);
                    self.pressed = None;
                    let status = self.status_column();
                    return self.open(key, status);
                }
                if !held.command && !held.shift && list.selection.contains(&key) {
                    list.focus = Some(key);
                    self.deferred = Some(key);
                } else {
                    list.click(key, held.command, held.shift);
                }
                self.dirty = true;
                None
            }
            Target::List => {
                self.focus = Focus::List;
                let held = self.held;
                if let Some(list) = self.list_mut()
                    && !held.command
                    && !held.shift
                {
                    list.choose([]);
                    self.dirty = true;
                }
                None
            }
            Target::Button(command) => {
                self.focus = Focus::Button(command);
                None
            }
            Target::Scroll(vertical, part) => {
                self.scroll(vertical, part);
                None
            }
            _ => None,
        }
    }

    pub(super) fn release(&mut self) -> Option<Effect> {
        let outline = self.outline();
        let landing = self.landing();
        let regroup = self.regroup();
        let pressed = self.pressed.take();
        let moved = std::mem::take(&mut self.moved);
        self.alone = false;
        let deferred = self.deferred.take();
        let target = self.hit();
        if let Some(menu) = &mut self.menu {
            let outcome = menu.release(target);
            return self.follow(outcome);
        }
        if let Some((area, _)) = outline {
            if let Some(sheet) = self.sheets.last_mut() {
                sheet.at = Point::new(area.x, area.y);
                return None;
            }
            let still = Rect::new(0, 0, self.size.width, self.size.height);
            return (area != still).then_some(Effect::Place(area));
        }
        if let Some(landing) = landing {
            self.land(landing);
            return None;
        }
        self.home();
        if let Some(regroup) = regroup {
            self.rehome(regroup);
            return None;
        }
        if let (Some(Target::Divider(column)), Some(track)) = (pressed, self.track(pressed))
            && let Some(inside) = self.inside()
            && let Some(list) = self.list_mut()
        {
            let start = inside.x - list.scrolled[0] + list.start(column);
            if let Some(width) = list.widths.get_mut(column) {
                *width = track.x - start;
                self.dirty = true;
            }
            return None;
        }
        if let Some(sheet) = self.sheets.last_mut() {
            let part = |target| match target {
                Some(Target::Dialog(part)) => Some(part),
                _ => None,
            };
            let outcome = sheet.dialog.release(part(pressed), part(target));
            return self.answer(outcome);
        }
        if let Some(key) = deferred.filter(|_| !moved)
            && let Some(list) = self.list_mut()
        {
            list.choose([key]);
            self.dirty = true;
        }
        if moved || pressed != target {
            return None;
        }
        match target? {
            Target::Minimize => Some(self.minimize()),
            Target::Zoom => Some(self.zoom()),
            Target::Close => Some(Effect::Close),
            Target::Button(command) => self.run(command),
            Target::Column(column) => {
                if let Some(list) = self.list_mut() {
                    list.sort(column);
                    self.dirty = true;
                }
                None
            }
            _ => None,
        }
    }

    pub(super) fn wheel(&mut self, by: Point) {
        let pointer = self.pointer;
        if self.menu.is_some() || !self.sheets.is_empty() {
            return;
        }
        let (bands, ..) = self.layout();
        if let Some(depth) = bands.iter().position(|band| band.contains(pointer)) {
            let delta = if by.x == 0 { by.y } else { by.x };
            let swiped = delta
                + if self.swiped.signum() == delta.signum() {
                    self.swiped
                } else {
                    0
                };
            let steps = swiped / SWIPE;
            self.swiped = swiped - steps * SWIPE;
            for _ in 0..steps.abs() {
                self.spin(depth, steps > 0);
            }
            return;
        }
        let Some(inside) = self.inside().filter(|inside| inside.contains(pointer)) else {
            return;
        };
        let spun = self.spun + by.y;
        let Some(list) = self.list_mut() else {
            return;
        };
        let pitch = list::ROW;
        let rows = spun / pitch;
        list.nudge(inside, true, rows);
        list.scroll(inside, false, |bar| bar.position + by.x);
        self.spun = spun - rows * pitch;
        self.dirty = true;
    }

    pub(super) fn type_in(&mut self, text: &str, paste: bool) {
        if let Some(sheet) = self.sheets.last_mut() {
            sheet.dialog.type_in(text, paste);
            self.blink = self.now.timestamp();
        }
    }

    pub(super) fn key(&mut self, keystroke: Keystroke) -> Option<Effect> {
        let Keystroke { key, held } = keystroke;
        let key = match key {
            Key::Character(letter) => {
                Key::Character(letter.to_lowercase().next().unwrap_or(letter))
            }
            key => key,
        };
        self.blink = self.now.timestamp();
        self.alt = held.alt;
        if self.dragging() {
            if key == Key::Escape {
                self.pressed = None;
                self.moved = false;
                self.home();
            }
            return None;
        }
        self.alone = key == Key::Alt;
        if key == Key::Alt {
            let plain = !(held.command || held.control || held.shift);
            self.cues.keys = true;
            self.alone = plain && self.menu.take().is_none();
            return None;
        }
        if let Some(menu) = &mut self.menu {
            let outcome = menu.key(key);
            return self.follow(outcome);
        }
        if let Some(sheet) = self.sheets.last_mut() {
            if let Some(reply) = sheet.dialog.pressable(key) {
                self.keyed = Some((key, Target::Dialog(Part::Reply(reply))));
                return None;
            }
            let outcome = sheet.dialog.key(Keystroke::new(key, held), &mut self.cues);
            return self.answer(outcome);
        }
        if self.keyed.is_some() {
            return None;
        }
        if let Some(&command) = self.settings.shortcuts.get(&Keystroke::new(key, held)) {
            return if self.enabled(command) {
                self.run(command)
            } else {
                Some(Effect::Beep)
            };
        }
        let listed = self.list().is_some();
        match key {
            Key::F10 => self.menu = Some(Tracking::new(self.menus(), 0, false, true)),
            Key::Character(letter) if held.alt => return self.accelerate(letter),
            Key::Character(letter) if held.command => return self.shortcut(letter, held.shift),
            Key::Tab if held.control => self.flip(self.strip_in_focus(), !held.shift),
            Key::Tab if self.focus == Focus::List && listed => return self.indent(!held.shift),
            Key::Tab | Key::Left | Key::Right if matches!(self.focus, Focus::Tabs(_)) => {
                let forth = if key == Key::Tab {
                    !held.shift
                } else {
                    key == Key::Right
                };
                self.flip(self.strip_in_focus(), forth);
            }
            Key::Tab => self.cycle_focus(held.shift),
            Key::Up | Key::Down if matches!(self.focus, Focus::Tabs(_)) => {
                self.descend(key == Key::Down);
            }
            Key::Up
                if listed
                    && !held.shift
                    && self.focus == Focus::List
                    && self
                        .list()
                        .is_some_and(|list| list.index().is_none_or(|row| row == 0)) =>
            {
                self.descend(false);
            }
            Key::Space if self.focus == Focus::List && !self.typing() => return self.toggle(),
            Key::F2 if listed => return self.rename(),
            Key::Up | Key::Down | Key::Home | Key::End | Key::PageUp | Key::PageDown if listed => {
                let key = match key {
                    Key::Up if held.command => Key::Home,
                    Key::Down if held.command => Key::End,
                    key => key,
                };
                self.travel(key, held.shift);
            }
            Key::Space | Key::Enter if matches!(self.focus, Focus::Button(_)) => {
                if let Focus::Button(command) = self.focus {
                    self.keyed = Some((key, Target::Button(command)));
                }
            }
            Key::Enter => return self.enter(held.command),
            Key::Backspace | Key::Delete if listed && self.focus == Focus::List => {
                return self.remove();
            }
            Key::Backspace | Key::Delete => return Some(Effect::Beep),
            _ => {}
        }
        None
    }

    fn toggle(&mut self) -> Option<Effect> {
        if self.kind() != Kind::Project || self.list().is_none_or(|list| list.focus.is_none()) {
            return Some(Effect::Beep);
        }
        self.cycle();
        None
    }

    fn rename(&mut self) -> Option<Effect> {
        let key = self.list().and_then(|list| list.focus);
        key.map_or(Some(Effect::Beep), |key| self.open(key, false))
    }

    fn enter(&mut self, command: bool) -> Option<Effect> {
        let focus = self
            .list()
            .and_then(|list| list.focus.filter(|key| list.selection.contains(key)));
        match focus {
            Some(key) if !command => self.open(key, false),
            _ => self.ask(),
        }
    }

    fn shortcut(&mut self, letter: char, shift: bool) -> Option<Effect> {
        let listed = self.list().is_some();
        match letter {
            'z' => return self.undo(!shift),
            'a' | 'c' | 'x' if !listed => return Some(Effect::Beep),
            'x' if self.focus != Focus::List => return Some(Effect::Beep),
            'a' => self.select_all(),
            'c' => return self.copy(),
            'x' => return self.cut(),
            '[' | '{' | ']' | '}' if shift => {
                self.flip(self.strip_in_focus(), matches!(letter, ']' | '}'));
            }
            _ => {}
        }
        None
    }

    pub(super) fn lift(&mut self, key: Key) -> Option<Effect> {
        if key == Key::Alt {
            self.alt = false;
        }
        if key == Key::Alt && std::mem::take(&mut self.alone) {
            if self.menu.take().is_none() && self.sheets.is_empty() {
                self.menu = Some(Tracking::new(self.menus(), 0, false, true));
            }
            return None;
        }
        let (pressed, target) = self.keyed?;
        if pressed != key {
            return None;
        }
        self.keyed = None;
        match target {
            Target::Dialog(Part::Reply(reply)) => self.answer(Outcome::Reply(reply)),
            Target::Button(command) if self.enabled(command) => self.run(command),
            _ => None,
        }
    }

    fn travel(&mut self, key: Key, extend: bool) {
        let Some(inside) = self.inside() else {
            return;
        };
        self.focus = Focus::List;
        self.cues.focus = true;
        let Some(list) = self.list_mut() else {
            return;
        };
        let Some(bottom) = list.rows.len().checked_sub(1) else {
            return;
        };
        let page = list::page(inside).saturating_sub(1).max(1);
        let index = match (key, list.index()) {
            (Key::End, _) => bottom,
            (Key::Up, Some(row)) => row.saturating_sub(1),
            (Key::Down, Some(row)) => (row + 1).min(bottom),
            (Key::PageUp, Some(row)) => row.saturating_sub(page),
            (Key::PageDown, Some(row)) => (row + page).min(bottom),
            _ => 0,
        };
        list.go(index, extend);
        list.reveal(inside);
        self.dirty = true;
    }

    fn select_all(&mut self) {
        let keys: Vec<u32> = self
            .list()
            .map(|list| list.rows.iter().map(|row| row.key).collect())
            .unwrap_or_default();
        let keys: Vec<u32> = keys
            .into_iter()
            .filter(|&key| self.draggable(key))
            .collect();
        self.focus = Focus::List;
        if let Some(list) = self.list_mut() {
            let focus = list.focus.filter(|key| keys.contains(key));
            list.choose(keys);
            list.focus = focus.or(list.focus);
            self.dirty = true;
        }
    }

    pub(super) fn seek(&mut self, text: &str) {
        let now = self.now.timestamp();
        if !self.typing() {
            self.search.clear();
        }
        if self.alt || self.list().is_none() || (self.search.is_empty() && text.trim().is_empty()) {
            return;
        }
        self.searched = now;
        self.search.push_str(&text.to_lowercase());
        let search = self.search.clone();
        let Some(inside) = self.inside() else {
            return;
        };
        let Some(list) = self.list_mut() else {
            return;
        };
        let found = list.rows.iter().find(|row| {
            row.cells
                .first()
                .is_some_and(|name| name.to_lowercase().starts_with(&search))
        });
        if let Some(key) = found.map(|row| row.key) {
            list.choose([key]);
            list.reveal(inside);
            self.focus = Focus::List;
            self.dirty = true;
        }
    }

    pub(super) const fn dragging(&self) -> bool {
        self.moved && self.pressed.is_some()
    }

    pub(super) fn typing(&self) -> bool {
        let elapsed = self.now.timestamp().duration_since(self.searched);
        !self.search.is_empty() && (0..i128::from(super::SEARCH)).contains(&elapsed.as_millis())
    }

    fn accelerate(&mut self, letter: char) -> Option<Effect> {
        let menus = self.menus();
        if let Some(title) = menus
            .iter()
            .position(|menu| controls::accelerator(menu.title) == Some(letter))
        {
            self.menu = Some(Tracking::new(menus, title, true, true));
            return None;
        }
        let &(_, command) = self.kind().buttons().1.iter().find(|&&(label, command)| {
            controls::accelerator(label) == Some(letter) && self.enabled(command)
        })?;
        self.run(command)
    }

    fn scroll(&mut self, vertical: bool, part: Bit) {
        let pointer = self.pointer;
        let Some(inside) = self.inside() else {
            return;
        };
        let Some(list) = self.list_mut() else {
            return;
        };
        match part {
            Bit::Back => list.nudge(inside, vertical, -1),
            Bit::Forth => list.nudge(inside, vertical, 1),
            Bit::PageBack => list.turn(inside, vertical, -1),
            Bit::PageForth => list.turn(inside, vertical, 1),
            Bit::Thumb => {
                if let Some(thumb) =
                    list.layout(inside).bars[usize::from(vertical)].and_then(|bar| bar.thumb())
                {
                    self.grab = Point::new(pointer.x - thumb.x, pointer.y - thumb.y);
                }
            }
        }
        self.dirty = true;
    }

    fn cycle_focus(&mut self, back: bool) {
        let stops = self.stops();
        let index = stops
            .iter()
            .position(|&stop| stop == self.focus)
            .unwrap_or(0);
        let step = if back { stops.len() - 1 } else { 1 };
        self.focus = stops[(index + step) % stops.len()];
        self.cues.focus = true;
    }

    fn stops(&self) -> Vec<Focus> {
        let mut stops = Vec::new();
        if self.list().is_some() {
            stops.push(Focus::List);
            stops.extend(
                self.kind()
                    .buttons()
                    .1
                    .iter()
                    .filter(|(_, command)| self.enabled(*command))
                    .map(|&(_, command)| Focus::Button(command)),
            );
        }
        stops.push(Focus::Tabs(0));
        stops
    }

    pub(super) fn zoom(&mut self) -> Effect {
        let window = self.state.window.get_or_insert(crate::state::Placement {
            frame: Rect::default(),
            zoomed: false,
        });
        window.zoomed = !window.zoomed;
        self.dirty = true;
        Effect::Zoom(window.zoomed)
    }

    fn minimize(&self) -> Effect {
        if self.state.options.contains(&Command::HideWhenMinimized) {
            Effect::Hide
        } else {
            Effect::Minimize
        }
    }

    pub(super) fn track(&self, pressed: Option<crate::ui::Target>) -> Option<Rect> {
        let Some(Target::Divider(column)) = pressed else {
            return None;
        };
        let inside = self.inside()?;
        let list = self.list()?;
        let start = inside.x - list.scrolled[0] + list.start(column);
        let x = self.pointer.x.min(inside.right() - 1).max(start);
        Some(Rect::new(x, inside.y, 1, inside.height))
    }

    pub(super) fn drift(&self) -> i32 {
        if !self.moved || !matches!(self.pressed, Some(Target::Row(_))) {
            return 0;
        }
        let (Some(inside), Some(list)) = (self.inside(), self.list()) else {
            return 0;
        };
        let body = list.layout(inside).body;
        let Point { x, y } = self.pointer;
        if x < body.x || x >= body.right() {
            0
        } else if (inside.y..body.y + EDGE).contains(&y) {
            -1
        } else {
            i32::from((body.bottom() - EDGE..body.bottom()).contains(&y))
        }
    }

    pub(super) fn inside(&self) -> Option<Rect> {
        self.targets
            .iter()
            .find(|&&(_, target)| target == Target::List)
            .map(|&(area, _)| area)
    }

    pub(super) fn strip(&self) -> Rect {
        let client = chrome::client(self.size);
        let status = chrome::status_area(self.size);
        Rect::sides(client.x + 5, client.y + 6, client.right() - 5, status.y - 5)
    }

    pub(super) fn status_column(&self) -> bool {
        let (Some(inside), Some(list)) = (self.inside(), self.list()) else {
            return false;
        };
        self.pointer.x >= inside.x - list.scrolled[0] + list.start(1)
    }
}
