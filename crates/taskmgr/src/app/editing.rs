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

use std::collections::BTreeSet;

use crate::app::{App, Effect, Landing, Place};
use crate::command::Kind;
use crate::dialog::{Notice, Purpose};
use crate::list::{INDENT, Mark, Row};
use crate::state::{Item, Node, SYSTEM, Task, tidy};
use crate::ui::Target;

struct Slot {
    order: Vec<u32>,
    before: usize,
    level: usize,
}

impl App {
    pub(super) fn draggable(&self, key: u32) -> bool {
        match self.kind() {
            Kind::Inbox => self
                .state
                .current()
                .inbox
                .iter()
                .any(|item| item.pid == key),
            Kind::Project => true,
            Kind::Performance => false,
        }
    }

    pub(super) fn landing(&self) -> Option<Landing> {
        if !self.hauling() {
            return None;
        }
        match self.hit()? {
            Target::Tab(depth, index) => {
                let place = self.tab_place(depth, index)?;
                (Some(&place) != self.source().as_ref()).then_some(Landing::Page(place))
            }
            Target::Row(_) | Target::List if self.abroad() => self.spot().map(Landing::Into),
            Target::Row(_) | Target::List => self.spot().map(Landing::Mark),
            _ => None,
        }
    }

    pub(super) fn cargo(&self) -> Option<(BTreeSet<u32>, usize, usize)> {
        if let Some(spring) = self.spring.as_ref().filter(|_| self.abroad()) {
            return Some((BTreeSet::new(), spring.head, spring.depth));
        }
        let list = self.list()?;
        let block = self.block();
        let mut moving = list.rows.iter().filter(|row| block.contains(&row.key));
        let head = moving.next()?.level;
        let tail = moving.next_back().map_or(head, |row| row.level);
        Some((block, head, tail.saturating_sub(head)))
    }

    fn spot(&self) -> Option<Mark> {
        let list = self.list()?;
        let index = list.landing(self.inside()?, self.pointer.y);
        let (block, head, depth) = self.cargo()?;
        let staying = |row: &&Row| !block.contains(&row.key) && self.draggable(row.key);
        let above = list.rows[..index].iter().rev().find(staying);
        let below = list.rows[index..].iter().find(staying);
        let ceiling = above.map_or(0, |row| row.level + 1);
        let floor = below.map_or(0, |row| row.level.saturating_sub(1 + depth));
        let shift = (self.pointer.x - self.anchor.x + INDENT / 2).div_euclid(INDENT);
        let wanted = i32::try_from(head).ok()? + shift;
        let wanted = usize::try_from(wanted.max(0)).unwrap_or(0);
        Some(Mark {
            index,
            level: wanted.max(floor).min(ceiling),
        })
    }

    pub(super) fn block(&self) -> BTreeSet<u32> {
        let Some(list) = self.list() else {
            return BTreeSet::new();
        };
        let mut block = BTreeSet::new();
        let mut within: Option<usize> = None;
        for row in &list.rows {
            if within.is_some_and(|level| row.level > level) {
                block.insert(row.key);
                continue;
            }
            within = None;
            if list.selection.contains(&row.key) {
                block.insert(row.key);
                within = Some(row.level);
            }
        }
        block
    }

    pub(super) fn land(&mut self, landing: Landing) {
        let slot = match landing {
            Landing::Into(mark) => self.slot(mark),
            _ => None,
        };
        self.home();
        self.refresh();
        let moving = self.block();
        self.record(|app| match (landing, slot) {
            (Landing::Page(place), _) => app.transfer(&moving, &place),
            (Landing::Mark(mark), _) => app.reorder(&moving, mark),
            (Landing::Into(_), Some((place, slot))) => {
                app.transfer(&moving, &place);
                app.seat(&place, &slot);
            }
            (Landing::Into(_), None) => {}
        });
        self.sampled = None;
    }

    fn slot(&self, mark: Mark) -> Option<(Place, Slot)> {
        let list = self.list()?;
        let before = list.rows[..mark.index.min(list.rows.len())]
            .iter()
            .filter(|row| self.draggable(row.key))
            .count();
        let slot = Slot {
            order: self.order(),
            before,
            level: mark.level,
        };
        Some((self.place()?, slot))
    }

    fn seat(&mut self, place: &Place, slot: &Slot) {
        let workspace = self.state.current_mut();
        let (moved, list) = match place {
            Place::Inbox => {
                let moving = workspace.list.selection.clone();
                let moved = relocate(
                    &mut workspace.inbox,
                    &slot.order,
                    &moving,
                    slot.before,
                    slot.level,
                );
                (moved, &mut workspace.list)
            }
            Place::Project(path) => {
                let Some(project) = workspace.project_at_mut(path) else {
                    return;
                };
                let moving = project.list.selection.clone();
                let moved = relocate(
                    &mut project.tasks,
                    &slot.order,
                    &moving,
                    slot.before,
                    slot.level,
                );
                (moved, &mut project.list)
            }
        };
        if moved {
            list.sorted = None;
        }
    }

    fn order(&self) -> Vec<u32> {
        self.list()
            .map(|list| list.rows.iter().map(|row| row.key).collect())
            .unwrap_or_default()
    }

    fn reorder(&mut self, moving: &BTreeSet<u32>, mark: Mark) {
        let Some(list) = self.list() else {
            return;
        };
        let before = list.rows[..mark.index.min(list.rows.len())]
            .iter()
            .filter(|row| !moving.contains(&row.key) && self.draggable(row.key))
            .count();
        let order = self.order();
        let level = mark.level;
        let changed = match self.kind() {
            Kind::Inbox => relocate(
                &mut self.state.current_mut().inbox,
                &order,
                moving,
                before,
                level,
            ),
            Kind::Project => self
                .state
                .current_mut()
                .active_mut()
                .is_some_and(|project| relocate(&mut project.tasks, &order, moving, before, level)),
            Kind::Performance => false,
        };
        if changed && let Some(list) = self.list_mut() {
            list.sorted = None;
        }
        self.dirty |= changed;
    }

    fn transfer(&mut self, moving: &BTreeSet<u32>, place: &Place) {
        let next = self.successor(moving);
        let order = self.order();
        let carried = match self.kind() {
            Kind::Inbox => take(&mut self.state.current_mut().inbox, &order, moving),
            Kind::Project => self
                .state
                .current_mut()
                .active_mut()
                .map(|project| take(&mut project.tasks, &order, moving))
                .unwrap_or_default(),
            Kind::Performance => return,
        };
        if let Some(list) = self.list_mut() {
            list.choose(next);
        }
        self.insert(place, carried, false);
    }

    pub(super) fn insert(&mut self, place: &Place, entries: Vec<(usize, String)>, after: bool) {
        let now = self.now.timestamp();
        let own = self.own();
        let base = entries.iter().map(|&(level, _)| level).min().unwrap_or(0);
        let workspace = self.state.current_mut();
        match place {
            Place::Inbox => {
                let focus = workspace.list.focus.filter(|_| after);
                let (at, level) = after_focus(&workspace.inbox, focus);
                let items: Vec<Item> = entries
                    .into_iter()
                    .map(|(depth, name)| Item {
                        level: level + depth - base,
                        ..workspace.add(name, now, own)
                    })
                    .collect();
                workspace.list.choose(items.iter().map(|item| item.pid));
                workspace.inbox.splice(at..at, items);
                tidy(&mut workspace.inbox);
            }
            Place::Project(path) => {
                let Some(project) = workspace.project_at_mut(path) else {
                    return;
                };
                let focus = project.list.focus.filter(|_| after);
                let (at, level) = after_focus(&project.tasks, focus);
                let first = project.fresh();
                let ids: Vec<u32> = (first..).take(entries.len()).collect();
                let tasks: Vec<Task> = ids
                    .iter()
                    .zip(entries)
                    .map(|(&id, (depth, name))| Task {
                        id,
                        name,
                        level: level + depth - base,
                        ..Task::default()
                    })
                    .collect();
                project.tasks.splice(at..at, tasks);
                project.repair();
                project.list.choose(ids);
            }
        }
        self.sampled = None;
        self.dirty = true;
    }

    fn successor(&self, gone: &BTreeSet<u32>) -> Option<u32> {
        let list = self.list()?;
        let first = list.rows.iter().position(|row| gone.contains(&row.key))?;
        let keep = |row: &&Row| !gone.contains(&row.key) && self.draggable(row.key);
        list.rows[first..]
            .iter()
            .find(keep)
            .or_else(|| list.rows[..first].iter().rev().find(keep))
            .map(|row| row.key)
    }

    pub(super) fn remove(&mut self) -> Option<Effect> {
        self.refresh();
        let selected = self.list()?.selected();
        if selected.is_empty() {
            return Some(Effect::Beep);
        }
        let gone: BTreeSet<u32> = self
            .block()
            .into_iter()
            .filter(|&key| self.draggable(key))
            .collect();
        if gone.is_empty() {
            return if selected.iter().any(|&pid| pid <= SYSTEM) {
                Some(self.notify(Notice::Untouchable))
            } else {
                Some(Effect::Close)
            };
        }
        self.record(|app| app.erase(&gone))
    }

    fn erase(&mut self, gone: &BTreeSet<u32>) -> Option<Effect> {
        let next = self.successor(gone);
        let kind = self.kind();
        let workspace = self.state.current_mut();
        match kind {
            Kind::Inbox => workspace.inbox.retain(|item| !gone.contains(&item.pid)),
            Kind::Project => {
                let project = workspace.active_mut()?;
                project.tasks.retain(|task| !gone.contains(&task.id));
                project.repair();
            }
            Kind::Performance => return None,
        }
        self.list_mut()?.choose(next);
        self.sampled = None;
        self.dirty = true;
        None
    }

    pub(super) fn indent(&mut self, deeper: bool) -> Option<Effect> {
        self.refresh();
        self.record(|app| app.nest(deeper))
    }

    fn nest(&mut self, deeper: bool) -> Option<Effect> {
        let order = self.order();
        let selection = self.list()?.selection.clone();
        let changed = match self.kind() {
            Kind::Inbox => deepen(
                &mut self.state.current_mut().inbox,
                &order,
                &selection,
                deeper,
            ),
            Kind::Project => self
                .state
                .current_mut()
                .active_mut()
                .is_some_and(|project| deepen(&mut project.tasks, &order, &selection, deeper)),
            Kind::Performance => false,
        };
        self.dirty |= changed;
        (!changed).then_some(Effect::Beep)
    }

    pub(super) fn copy(&mut self) -> Option<Effect> {
        self.refresh();
        let block = self.block();
        let list = self.list()?;
        let rows: Vec<&Row> = list
            .rows
            .iter()
            .filter(|row| block.contains(&row.key))
            .collect();
        let Some(base) = rows.iter().map(|row| row.level).min() else {
            return Some(Effect::Beep);
        };
        let lines: Vec<String> = rows
            .iter()
            .map(|row| {
                let name = self
                    .name(row.key)
                    .or_else(|| row.cells.first().map(String::as_str))
                    .unwrap_or_default();
                format!("{}{name}", "\t".repeat(row.level - base))
            })
            .collect();
        Some(Effect::Copy(lines.join("\n")))
    }

    fn name(&self, key: u32) -> Option<&str> {
        let workspace = self.state.current();
        match self.kind() {
            Kind::Inbox => workspace
                .inbox
                .iter()
                .find(|item| item.pid == key)
                .map(|item| item.name.as_str()),
            Kind::Project => {
                let project = workspace.active()?;
                Some(project.tasks[project.index(key)?].name.as_str())
            }
            Kind::Performance => None,
        }
    }

    pub(super) fn cut(&mut self) -> Option<Effect> {
        let copied = self.copy();
        if !matches!(copied, Some(Effect::Copy(_))) {
            return copied;
        }
        self.remove().or(copied)
    }

    pub(super) fn paste(&mut self, text: &str) -> Option<Effect> {
        let Some(place) = self.place() else {
            return Some(Effect::Beep);
        };
        let lines: Vec<(usize, String)> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let tabs = line.len() - line.trim_start_matches('\t').len();
                (tabs, line.trim().to_owned())
            })
            .collect();
        if lines.is_empty() {
            return Some(Effect::Beep);
        }
        let names = lines.iter().map(|(_, name)| name.as_str());
        if let Some(notice) = self.refusal(Purpose::Task, names) {
            return Some(self.notify(notice));
        }
        self.refresh();
        self.record(|app| app.insert(&place, lines, true));
        None
    }
}

fn arrange<T: Node>(nodes: &[T], order: &[u32]) -> Vec<T> {
    let mut nodes = nodes.to_vec();
    nodes.sort_by_key(|node| order.iter().position(|&key| key == node.key()));
    nodes
}

fn relocate<T: Node>(
    nodes: &mut Vec<T>,
    order: &[u32],
    moving: &BTreeSet<u32>,
    before: usize,
    level: usize,
) -> bool {
    let shown = arrange(nodes, order);
    let (mut block, mut rest): (Vec<T>, Vec<T>) = shown
        .iter()
        .cloned()
        .partition(|node| moving.contains(&node.key()));
    let first = block.first().map_or(0, Node::level);
    for node in &mut block {
        node.set_level((node.level() + level).saturating_sub(first));
    }
    let tail = rest.split_off(before.min(rest.len()));
    rest.extend(block);
    rest.extend(tail);
    tidy(&mut rest);
    let changed = rest != shown;
    if changed {
        *nodes = rest;
    }
    changed
}

fn take<T: Node>(
    nodes: &mut Vec<T>,
    order: &[u32],
    moving: &BTreeSet<u32>,
) -> Vec<(usize, String)> {
    let (gone, kept): (Vec<T>, Vec<T>) = arrange(nodes, order)
        .into_iter()
        .partition(|node| moving.contains(&node.key()));
    *nodes = kept;
    tidy(nodes);
    gone.iter()
        .map(|node| (node.level(), node.name().to_owned()))
        .collect()
}

fn deepen<T: Node>(
    nodes: &mut Vec<T>,
    order: &[u32],
    selection: &BTreeSet<u32>,
    deeper: bool,
) -> bool {
    let mut shown = arrange(nodes, order);
    let mut changed = false;
    let mut index = 0;
    while index < shown.len() {
        let level = shown[index].level();
        let end = index
            + 1
            + shown[index + 1..]
                .iter()
                .take_while(|node| node.level() > level)
                .count();
        let allowed = if deeper {
            index > 0 && level <= shown[index - 1].level()
        } else {
            level > 0
        };
        if selection.contains(&shown[index].key()) && allowed {
            for node in &mut shown[index..end] {
                let level = node.level();
                node.set_level(if deeper { level + 1 } else { level - 1 });
            }
            changed = true;
            index = end;
        } else {
            index += 1;
        }
    }
    if changed {
        *nodes = shown;
    }
    changed
}

fn after_focus<T: Node>(nodes: &[T], focus: Option<u32>) -> (usize, usize) {
    let Some(index) = focus.and_then(|key| nodes.iter().position(|node| node.key() == key)) else {
        return (nodes.len(), 0);
    };
    let level = nodes[index].level();
    let inside = nodes[index + 1..]
        .iter()
        .take_while(|node| node.level() > level)
        .count();
    (index + 1 + inside, level)
}
