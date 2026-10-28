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

use crate::app::{App, Place};
use crate::command::Kind;
use crate::pages::{self, Focus};
use crate::state::Page;
use crate::ui::Target;

impl App {
    pub(super) fn select(&mut self, depth: usize, index: usize) {
        let workspace = self.state.current_mut();
        let path = workspace.path();
        let mut target = path[..depth.min(path.len())].to_vec();
        if let Some(parent) = workspace.project_at_mut(&target) {
            parent.page = (index + 1).min(parent.projects.len());
        } else {
            workspace.page = index;
        }
        target.push(index);
        if let Some(project) = workspace.project_at_mut(&target) {
            project.page = 0;
        }
        self.dirty = true;
        self.uncover(depth, index);
        self.refocus();
    }

    pub(super) fn uncover(&mut self, depth: usize, index: usize) {
        let (bands, ..) = self.layout();
        if let Some(&area) = bands.get(depth) {
            let (titles, _) = self.titles(depth);
            let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
            self.first.resize(depth + 1, 0);
            self.first[depth] = pages::scrolled(area, &titles, self.first[depth], index);
        }
    }

    pub(super) fn go(&mut self, path: &[usize]) {
        let workspace = self.state.current_mut();
        if let Some(&top) = path.first() {
            workspace.page = top;
        }
        for end in 1..=path.len() {
            if let Some(project) = workspace.project_at_mut(&path[..end]) {
                project.page = path.get(end).map_or(0, |&child| child + 1);
            }
        }
        self.dirty = true;
        self.refocus();
    }

    pub(super) fn refocus(&mut self) {
        self.first.truncate(self.depth());
        if self.kind() == Kind::Performance {
            self.focus = Focus::Tabs(0);
        } else if let Focus::Tabs(level) = self.focus {
            self.focus = Focus::Tabs(level.min(self.depth() - 1));
        }
        self.refresh();
    }

    pub(super) fn flip(&mut self, depth: usize, forth: bool) {
        let (titles, selected) = self.titles(depth);
        let count = titles.len();
        if count == 0 {
            return;
        }
        let index = match (selected, forth) {
            (Some(index), true) => (index + 1) % count,
            (Some(index), false) => (index + count - 1) % count,
            (None, true) => 0,
            (None, false) => count - 1,
        };
        self.select(depth, index);
        self.focus = Focus::Tabs(depth);
        self.cues.focus = true;
    }

    pub(super) fn strip_in_focus(&self) -> usize {
        match self.focus {
            Focus::Tabs(depth) => depth,
            _ => 0,
        }
    }

    pub(super) fn descend(&mut self, down: bool) {
        let deepest = self.depth() - 1;
        self.focus = match (self.focus, down) {
            (Focus::Tabs(depth), true) if depth < deepest => Focus::Tabs(depth + 1),
            (Focus::Tabs(_), true) if self.list().is_some() => Focus::List,
            (Focus::Tabs(depth), false) => Focus::Tabs(depth.saturating_sub(1)),
            (Focus::List | Focus::Button(_), false) => Focus::Tabs(deepest),
            (focus, _) => focus,
        };
        self.cues.focus = true;
        if self.focus == Focus::List
            && let Some(list) = self.list_mut()
            && list.focus.is_none()
        {
            list.go(0, false);
        }
    }

    pub(super) fn spin(&mut self, depth: usize, forth: bool) {
        let (bands, ..) = self.layout();
        let Some(&area) = bands.get(depth) else {
            return;
        };
        let (titles, _) = self.titles(depth);
        let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
        let last = pages::last(area, &titles);
        self.first.resize(depth + 1, 0);
        let first = &mut self.first[depth];
        *first = if forth {
            (*first + 1).min(last)
        } else {
            first.saturating_sub(1)
        };
    }

    pub(super) fn shuffle(&mut self, depth: usize, from: usize) {
        let Some(Target::Tab(level, to)) = self.hit() else {
            return;
        };
        let rect = |index| {
            self.targets
                .iter()
                .find(|&&(_, target)| target == Target::Tab(depth, index))
                .map(|&(rect, _)| rect)
        };
        let (Some(source), Some(goal)) = (rect(from), rect(to)) else {
            return;
        };
        let past = if to > from {
            self.pointer.x >= goal.right() - source.width
        } else {
            self.pointer.x < goal.x + source.width
        };
        if level != depth || to == from || !past {
            return;
        }
        let workspace = self.state.current_mut();
        if depth == 0 {
            let page = workspace.pages.remove(from);
            workspace.pages.insert(to, page);
            workspace.page = to;
        } else {
            let path = workspace.path();
            let Some(project) = workspace.project_at_mut(&path[..depth]) else {
                return;
            };
            let child = project.projects.remove(from);
            project.projects.insert(to, child);
            project.page = to + 1;
        }
        self.pressed = Some(Target::Tab(depth, to));
        self.dirty = true;
    }

    pub(super) fn tab_project(&self, depth: usize, index: usize) -> Option<Vec<usize>> {
        match self.tab_place(depth, index)? {
            Place::Project(path) => Some(path),
            Place::Inbox => None,
        }
    }

    pub(super) fn tab_place(&self, depth: usize, index: usize) -> Option<Place> {
        let workspace = self.state.current();
        if depth == 0 {
            return match workspace.pages.get(index)? {
                Page::Inbox => Some(Place::Inbox),
                Page::Project(_) => Some(Place::Project(vec![index])),
                Page::Performance => None,
            };
        }
        let path = workspace.path();
        let mut target = path.get(..depth)?.to_vec();
        target.push(index);
        workspace.project_at(&target)?;
        Some(Place::Project(target))
    }
}
