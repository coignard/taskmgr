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

use crate::app::App;
use crate::ui::Target;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Regroup {
    Into {
        source: Vec<usize>,
        level: usize,
        index: usize,
        target: Vec<usize>,
    },
    Among {
        source: Vec<usize>,
        level: usize,
        gap: usize,
        parent: Vec<usize>,
    },
}

impl App {
    pub(super) fn regroup(&self) -> Option<Regroup> {
        let Some(Target::Tab(depth, index)) = self.pressed else {
            return None;
        };
        if !self.moved {
            return None;
        }
        let source = self.tab_project(depth, index)?;
        if self.alt {
            let Some(Target::Tab(level, index)) = self.hit() else {
                return None;
            };
            let target = self.tab_project(level, index)?;
            let home = target.starts_with(&source) || target == source[..depth];
            return (!home).then_some(Regroup::Into {
                source,
                level,
                index,
                target,
            });
        }
        let (bands, ..) = self.layout();
        let level = bands.iter().position(|band| band.contains(self.pointer))?;
        let parent = self.state.current().path().get(..level)?.to_vec();
        if level == depth || parent.starts_with(&source) {
            return None;
        }
        Some(Regroup::Among {
            source,
            level,
            gap: self.gap(level),
            parent,
        })
    }

    fn gap(&self, level: usize) -> usize {
        let mut tabs: Vec<_> = self
            .targets
            .iter()
            .filter_map(|&(rect, target)| match target {
                Target::Tab(depth, index) if depth == level => Some((index, rect)),
                _ => None,
            })
            .collect();
        tabs.sort_by_key(|&(index, _)| index);
        tabs.iter()
            .find(|(_, rect)| self.pointer.x < rect.x + rect.width / 2)
            .or(tabs.last())
            .map_or(0, |&(index, rect)| {
                index + usize::from(self.pointer.x >= rect.x + rect.width / 2)
            })
    }

    pub(super) fn rehome(&mut self, regroup: Regroup) {
        let (source, parent, index) = match regroup {
            Regroup::Into { source, target, .. } => (source, target, usize::MAX),
            Regroup::Among {
                source,
                parent,
                gap,
                ..
            } => (source, parent, gap),
        };
        self.record(|app| {
            let workspace = app.state.current_mut();
            let Some(project) = workspace.detach(&source) else {
                return;
            };
            let parent = shifted(&parent, &source);
            if let Some(path) = workspace.attach(&parent, index, project) {
                app.go(&path);
                app.uncover(path.len() - 1, path[path.len() - 1]);
            }
        });
        self.sampled = None;
    }
}

fn shifted(path: &[usize], removed: &[usize]) -> Vec<usize> {
    let mut path = path.to_vec();
    let at = removed.len() - 1;
    if path.len() > at && path[..at] == removed[..at] && path[at] > removed[at] {
        path[at] -= 1;
    }
    path
}
