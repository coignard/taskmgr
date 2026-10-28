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

use crate::app::{App, Effect, Place, Sheet};
use crate::command::{Command, Kind};
use crate::dialog::{About, Columns, Dialog, Message, Notice, Outcome, Prompt, Purpose, Reply};
use crate::geometry::{Point, Size};
use crate::menu::tracking::Outcome as Choice;
use crate::metric::Metric;
use crate::pages::Focus;
use crate::state::{Page, Project, Workspace};
use crate::system;
use crate::tag;

const HISTORY: usize = 100;

impl App {
    pub(super) fn follow(&mut self, choice: Choice) -> Option<Effect> {
        match choice {
            Choice::Close => {
                self.menu = None;
                None
            }
            Choice::Run(command) if self.enabled(command) => {
                self.menu = None;
                self.run(command)
            }
            Choice::Stay | Choice::Run(_) => None,
        }
    }

    pub(super) fn run(&mut self, command: Command) -> Option<Effect> {
        match command {
            Command::Exit => return Some(Effect::Close),
            Command::NewProject => self.prompt(Purpose::Project, String::new()),
            Command::NewSubproject => {
                self.state.current().active()?;
                self.prompt(Purpose::Subproject, String::new());
            }
            Command::RenameProject => self.rename_project(self.state.current().path()),
            Command::DeleteProject => {
                let path = self.state.current().path();
                self.state.current().project_at(&path)?;
                let nested = path.len() > 1;
                self.subject = path;
                return Some(self.notify(Notice::DeleteProject(nested)));
            }
            Command::NewWorkspace => self.prompt(Purpose::Workspace, String::new()),
            Command::RenameWorkspace => {
                let name = self.state.current().name.clone();
                self.prompt(Purpose::RenameWorkspace, name);
            }
            Command::DeleteWorkspace => {
                if self.state.workspaces.len() < 2 {
                    return Some(Effect::Beep);
                }
                return Some(self.notify(Notice::DeleteWorkspace(self.state.workspace)));
            }
            Command::NewTask => return self.ask(),
            Command::Workspace(ordinal) => {
                if !(1..=self.state.workspaces.len()).contains(&ordinal) {
                    return Some(Effect::Beep);
                }
                self.switch(ordinal - 1);
            }
            Command::About => {
                let about = About {
                    licensee: self.host.user.clone(),
                    memory: self.settings.memory,
                };
                self.show(Dialog::About(about));
            }
            Command::SelectColumns => {
                let columns = Columns::new(&self.state.current().columns);
                self.show(Dialog::Columns(columns));
            }
            Command::Refresh => self.sampled = None,
            Command::EndTask | Command::EndProcess => return self.remove(),
            Command::SwitchTo => self.cycle(),
            Command::Speed(_) => {
                let kind = mem::discriminant(&command);
                let options = &mut self.state.current_mut().options;
                options.retain(|other| mem::discriminant(other) != kind);
                options.insert(command);
                self.sampled = None;
                self.dirty = true;
            }
            Command::AlwaysOnTop
            | Command::HideWhenMinimized
            | Command::HideDockIcon
            | Command::HideSystemProcesses
            | Command::GraphPerProcessor
            | Command::KernelTimes => {
                let options = if command.local() {
                    &mut self.state.current_mut().options
                } else {
                    &mut self.state.options
                };
                if !options.remove(&command) {
                    options.insert(command);
                }
                self.dirty = true;
                self.refresh();
                match command {
                    Command::AlwaysOnTop => return Some(Effect::Float(self.floats())),
                    Command::HideDockIcon => return Some(Effect::Dock(self.docked())),
                    _ => {}
                }
            }
        }
        None
    }

    pub(super) fn answer(&mut self, outcome: Outcome) -> Option<Effect> {
        let reply = match outcome {
            Outcome::Stay => return None,
            Outcome::Copy(text) => return Some(Effect::Copy(text)),
            Outcome::Reply(reply) => reply,
        };
        self.keyed = None;
        if let Some(Sheet {
            dialog: Dialog::Prompt(prompt),
            ..
        }) = self.sheets.last()
            && reply == Reply::Ok
            && let Some(notice) = self.refusal(prompt.purpose, [prompt.text()])
        {
            return Some(self.notify(notice));
        }
        let sheet = self.sheets.pop()?;
        match (sheet.dialog, reply) {
            (Dialog::Prompt(prompt), Reply::Ok) => {
                let text = prompt.text().to_owned();
                self.accept(prompt.purpose, text);
            }
            (Dialog::Message(message), Reply::Yes) => match message.notice {
                Notice::DeleteProject(_) => {
                    let path = mem::take(&mut self.subject);
                    self.record(|app| app.delete_project(&path));
                }
                Notice::DeleteWorkspace(index) => self.delete_workspace(index),
                _ => {}
            },
            (Dialog::Columns(columns), Reply::Ok) => self.arrange(&columns.chosen()),
            _ => {}
        }
        None
    }

    pub(super) fn ask(&mut self) -> Option<Effect> {
        if !self.room(0) {
            return Some(self.notify(Notice::Memory));
        }
        let workspace = self.state.current();
        let text = workspace.recent.first().cloned().unwrap_or_default();
        self.prompt(Purpose::Task, text);
        None
    }

    pub(super) fn rename_project(&mut self, path: Vec<usize>) {
        let Some(project) = self.state.current().project_at(&path) else {
            return;
        };
        let name = project.name.clone();
        let nested = path.len() > 1;
        self.subject = path;
        self.prompt(Purpose::RenameProject(nested), name);
    }

    pub(super) fn open(&mut self, key: u32, status: bool) -> Option<Effect> {
        let workspace = self.state.current();
        let name = match self.kind() {
            Kind::Project if status => {
                self.cycle();
                return None;
            }
            Kind::Project => {
                let project = workspace.active()?;
                project.tasks[project.index(key)?].name.clone()
            }
            Kind::Inbox => {
                let Some(item) = workspace.inbox.iter().find(|item| item.pid == key) else {
                    return Some(Effect::Beep);
                };
                item.name.clone()
            }
            Kind::Performance => return None,
        };
        self.prompt(Purpose::Rename(key), name);
        None
    }

    pub(super) fn notify(&mut self, notice: Notice) -> Effect {
        self.show(Dialog::Message(Message::new(notice)));
        Effect::Alert
    }

    pub(super) fn room(&self, extra: u64) -> bool {
        let commit = system::commit(self.state.current(), self.now.timestamp());
        commit + extra <= self.settings.memory
    }

    pub(super) fn refusal<'a>(
        &self,
        purpose: Purpose,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Option<Notice> {
        if !matches!(purpose, Purpose::Task | Purpose::Rename(_)) {
            return None;
        }
        let names: Vec<&str> = names.into_iter().map(str::trim).collect();
        let licensed = |name: &str| {
            let (shown, _) = tag::split(name);
            ["taskmgr", "taskmgr.exe"]
                .iter()
                .any(|own| shown.eq_ignore_ascii_case(own))
        };
        if names.iter().any(|name| licensed(name)) {
            return Some(Notice::License);
        }
        let cost = names.iter().map(|name| system::cost(name)).sum();
        (purpose == Purpose::Task && !self.room(cost)).then_some(Notice::Memory)
    }

    pub(super) fn record<R>(&mut self, edit: impl FnOnce(&mut Self) -> R) -> R {
        let before = self.state.current().snapshot();
        let result = edit(self);
        let workspace = self.state.current_mut();
        if workspace.snapshot() != before {
            workspace.undo.push(before);
            workspace.redo.clear();
            if workspace.undo.len() > HISTORY {
                workspace.undo.remove(0);
            }
        }
        result
    }

    pub(super) fn undo(&mut self, back: bool) -> Option<Effect> {
        let workspace = self.state.current_mut();
        let from = if back {
            &mut workspace.undo
        } else {
            &mut workspace.redo
        };
        let Some(snapshot) = from.pop() else {
            return Some(Effect::Beep);
        };
        let mut probe = Workspace::default();
        probe.restore(&snapshot);
        workspace.page = probe.page;
        let current = workspace.snapshot();
        if back {
            workspace.redo.push(current);
        } else {
            workspace.undo.push(current);
        }
        workspace.restore(&snapshot);
        self.refocus();
        self.focus = Focus::List;
        self.sampled = None;
        None
    }

    pub(super) fn cycle(&mut self) {
        self.record(Self::shift_status);
    }

    fn shift_status(&mut self) {
        let Some(project) = self.state.current_mut().active_mut() else {
            return;
        };
        let Some(focus) = project.list.focus.and_then(|id| project.index(id)) else {
            return;
        };
        let status = project.tasks[focus].status.next();
        for task in &mut project.tasks {
            if project.list.selection.contains(&task.id) {
                task.status = status;
            }
        }
        self.sampled = None;
        self.dirty = true;
    }

    fn accept(&mut self, purpose: Purpose, text: String) {
        match purpose {
            Purpose::Workspace | Purpose::RenameWorkspace => self.settle(purpose, text),
            _ => self.record(|app| app.settle(purpose, text)),
        }
        self.sampled = None;
        self.dirty = true;
    }

    fn settle(&mut self, purpose: Purpose, text: String) {
        let now = self.now.timestamp();
        let workspace = self.state.current_mut();
        match purpose {
            Purpose::Task => {
                workspace.remember(&text);
                let place = self.place().unwrap_or(Place::Inbox);
                self.insert(&place, vec![(0, text)], true);
                self.focus = Focus::List;
            }
            Purpose::Rename(key) => {
                workspace.remember(&text);
                if self.kind() == Kind::Inbox {
                    let inbox = &mut self.state.current_mut().inbox;
                    if let Some(item) = inbox.iter_mut().find(|item| item.pid == key) {
                        item.name = text;
                    }
                } else if let Some(project) = self.state.current_mut().active_mut()
                    && let Some(index) = project.index(key)
                {
                    project.tasks[index].name = text;
                }
            }
            Purpose::Project => {
                let at = workspace.page + 1;
                workspace.pages.insert(
                    at,
                    Page::Project(Box::new(Project {
                        name: text,
                        ..Project::default()
                    })),
                );
                self.select(0, at);
                self.focus = Focus::List;
            }
            Purpose::Subproject => {
                let depth = workspace.path().len();
                let Some(project) = workspace.active_mut() else {
                    return;
                };
                project.projects.push(Project {
                    name: text,
                    ..Project::default()
                });
                let index = project.projects.len() - 1;
                self.uncover(depth, index);
                self.refocus();
            }
            Purpose::RenameProject(_) => {
                let path = mem::take(&mut self.subject);
                if let Some(project) = self.state.current_mut().project_at_mut(&path) {
                    project.name = text;
                }
            }
            Purpose::Workspace => {
                self.state.workspaces.push(Workspace::new(text, now));
                self.switch(self.state.workspaces.len() - 1);
            }
            Purpose::RenameWorkspace => workspace.name = text,
        }
    }

    fn delete_workspace(&mut self, index: usize) {
        let count = self.state.workspaces.len();
        if count < 2 || index >= count {
            return;
        }
        self.state.workspaces.remove(index);
        self.switch(index.saturating_sub(1));
    }

    fn delete_project(&mut self, path: &[usize]) {
        let workspace = self.state.current_mut();
        match path {
            [page] => {
                if workspace.project(*page).is_none() {
                    return;
                }
                workspace.pages.remove(*page);
                let page = (*page).min(workspace.pages.len() - 1);
                self.select(0, page);
                self.focus = Focus::Tabs(0);
            }
            [parent @ .., child] => {
                let Some(project) = workspace.project_at_mut(parent) else {
                    return;
                };
                if *child >= project.projects.len() {
                    return;
                }
                project.projects.remove(*child);
                let depth = parent.len() - 1;
                self.select(depth, parent[depth]);
                self.focus = Focus::Tabs(depth);
            }
            [] => {}
        }
        self.sampled = None;
    }

    fn arrange(&mut self, chosen: &[Metric]) {
        let workspace = self.state.current_mut();
        let before = mem::replace(&mut workspace.columns, chosen.to_vec());
        let list = &mut workspace.list;
        let widths: Vec<i32> = chosen
            .iter()
            .map(|metric| {
                before
                    .iter()
                    .position(|other| other == metric)
                    .and_then(|index| list.widths.get(index).copied())
                    .unwrap_or_else(|| metric.column().width)
            })
            .collect();
        list.sorted = list.sorted.and_then(|(column, ascending)| {
            let metric = before.get(column)?;
            let column = chosen.iter().position(|other| other == metric)?;
            Some((column, ascending))
        });
        list.widths = widths;
        self.dirty = true;
        self.refresh();
    }

    fn prompt(&mut self, purpose: Purpose, text: String) {
        let recent = match purpose {
            Purpose::Task | Purpose::Rename(_) => self.state.current().recent.clone(),
            _ => Vec::new(),
        };
        self.show(Dialog::Prompt(Prompt::new(purpose, text, recent)));
    }

    fn show(&mut self, dialog: Dialog) {
        self.menu = None;
        self.keyed = None;
        self.blink = self.now.timestamp();
        let size = dialog.size();
        let at = match self.sheets.last() {
            Some(sheet) => {
                let below = sheet.dialog.size();
                Point::new(
                    sheet.at.x + (below.width - size.width) / 2,
                    sheet.at.y + (below.height - size.height) / 2,
                )
            }
            None => self.centered(size),
        };
        self.sheets.push(Sheet { dialog, at });
    }

    pub(super) fn switch(&mut self, index: usize) {
        self.state.workspace = index;
        self.sampled = None;
        self.focus = Focus::List;
        self.first.clear();
        self.refocus();
        self.advance();
        self.dirty = true;
    }

    fn centered(&self, size: Size) -> Point {
        Point::new(
            (self.size.width - size.width) / 2,
            (self.size.height - size.height) / 2,
        )
    }
}
