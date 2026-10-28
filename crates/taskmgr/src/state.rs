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

use std::collections::{BTreeMap, BTreeSet};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Speed};
use crate::geometry::Rect;
use crate::keys::{Key, Keystroke, Modifiers};
use crate::list::List;
use crate::metric::{self, Metric};

const VERSION: u32 = 1;
const FIRST: &str = "Default";
const MEMORY: u64 = 523_828;
pub(crate) const SYSTEM: u32 = 8;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct State {
    pub(crate) version: u32,
    pub(crate) window: Option<Placement>,
    #[serde(deserialize_with = "lenient")]
    pub(crate) options: BTreeSet<Command>,
    pub(crate) workspace: usize,
    pub(crate) workspaces: Vec<Workspace>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub frame: Rect,
    pub zoomed: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Workspace {
    pub(crate) name: String,
    pub(crate) created: Timestamp,
    #[serde(deserialize_with = "lenient")]
    pub(crate) options: BTreeSet<Command>,
    #[serde(deserialize_with = "lenient")]
    pub(crate) columns: Vec<Metric>,
    pub(crate) next: u32,
    pub(crate) inbox: Vec<Item>,
    pub(crate) list: List,
    pub(crate) pages: Vec<Page>,
    pub(crate) page: usize,
    pub(crate) recent: Vec<String>,
    pub(crate) history: History,
    pub(crate) peak: u64,
    #[serde(skip)]
    pub(crate) undo: Vec<String>,
    #[serde(skip)]
    pub(crate) redo: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Page {
    Inbox,
    Performance,
    Project(Box<Project>),
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Project {
    pub(crate) name: String,
    pub(crate) tasks: Vec<Task>,
    pub(crate) list: List,
    pub(crate) projects: Vec<Project>,
    pub(crate) page: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Task {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) status: Status,
    pub(crate) level: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
    #[default]
    Stopped,
    Running,
    NotResponding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Item {
    pub(crate) pid: u32,
    pub(crate) name: String,
    pub(crate) added: Timestamp,
    #[serde(default)]
    pub(crate) level: usize,
}

pub(crate) trait Node: Clone + PartialEq {
    fn key(&self) -> u32;
    fn level(&self) -> usize;
    fn set_level(&mut self, level: usize);
    fn name(&self) -> &str;
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct History {
    pub(crate) until: Option<Timestamp>,
    #[serde(with = "samples")]
    pub(crate) samples: Vec<Sample>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Sample {
    pub(crate) cpu: u8,
    pub(crate) kernel: u8,
    pub(crate) memory: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Settings {
    pub(crate) shortcuts: BTreeMap<Keystroke, Command>,
    pub(crate) toggle: Option<Keystroke>,
    pub(crate) memory: u64,
}

impl Default for Settings {
    fn default() -> Self {
        let command = Modifiers {
            command: true,
            ..Modifiers::default()
        };
        let shift = Modifiers {
            shift: true,
            ..command
        };
        let mut shortcuts: BTreeMap<Keystroke, Command> = ('1'..='9')
            .zip(1..)
            .map(|(digit, ordinal)| {
                let keystroke = Keystroke::new(Key::Character(digit), command);
                (keystroke, Command::Workspace(ordinal))
            })
            .collect();
        shortcuts.insert(
            Keystroke::new(Key::Character('n'), command),
            Command::NewProject,
        );
        shortcuts.insert(
            Keystroke::new(Key::Character('n'), shift),
            Command::NewSubproject,
        );
        Self {
            shortcuts,
            toggle: Some(Keystroke::new(
                Key::Character(']'),
                Modifiers { alt: true, ..shift },
            )),
            memory: MEMORY,
        }
    }
}

impl Settings {
    pub(crate) fn adopt(&mut self, defaults: Self) -> bool {
        let mut changed = false;
        for (keystroke, command) in defaults.shortcuts {
            let bound = self.shortcuts.values().any(|&other| other == command);
            if !bound && !self.shortcuts.contains_key(&keystroke) {
                self.shortcuts.insert(keystroke, command);
                changed = true;
            }
        }
        changed
    }
}

impl State {
    pub(crate) fn repair(&mut self, now: Timestamp) {
        self.version = VERSION;
        if self.workspaces.is_empty() {
            self.workspaces.push(Workspace::new(FIRST.to_owned(), now));
        }
        self.workspace = self.workspace.min(self.workspaces.len() - 1);
        let local: Vec<Command> = self
            .options
            .iter()
            .copied()
            .filter(|command| command.local())
            .collect();
        self.options.retain(|command| !command.local());
        for workspace in &mut self.workspaces {
            if workspace.options.is_empty() {
                workspace.options.extend(&local);
            }
            workspace.repair(now);
        }
    }

    pub(crate) fn current(&self) -> &Workspace {
        &self.workspaces[self.workspace]
    }

    pub(crate) fn current_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.workspace]
    }
}

impl Workspace {
    pub(crate) fn new(name: String, now: Timestamp) -> Self {
        let mut workspace = Self {
            name,
            created: now,
            ..Self::default()
        };
        workspace.repair(now);
        workspace
    }

    fn repair(&mut self, now: Timestamp) {
        if self.created == Timestamp::default() {
            self.created = now;
        }
        if !self
            .options
            .iter()
            .any(|command| matches!(command, Command::Speed(_)))
        {
            self.options.insert(Command::Speed(Speed::Normal));
            self.options.insert(Command::GraphPerProcessor);
        }
        if self.columns.is_empty() {
            self.columns = metric::DEFAULT.to_vec();
        }
        self.columns.retain(|&metric| metric != Metric::Name);
        self.columns.insert(0, Metric::Name);
        self.columns.sort();
        self.columns.dedup();
        let highest = self.inbox.iter().map(|item| item.pid).max();
        self.next = self
            .next
            .max(SYSTEM + 1)
            .max(highest.map_or(0, |pid| pid + 1));
        tidy(&mut self.inbox);
        let mut seen = [false; 2];
        self.pages.retain(|page| {
            let slot = match page {
                Page::Inbox => 0,
                Page::Performance => 1,
                Page::Project(_) => return true,
            };
            !std::mem::replace(&mut seen[slot], true)
        });
        if !seen[0] {
            self.pages.insert(0, Page::Inbox);
        }
        if !seen[1] {
            self.pages.push(Page::Performance);
        }
        self.page = self.page.min(self.pages.len() - 1);
        for page in &mut self.pages {
            if let Page::Project(project) = page {
                project.repair();
            }
        }
    }

    pub(crate) fn add(&mut self, name: String, now: Timestamp, reserved: u32) -> Item {
        if self.next == reserved {
            self.next += 1;
        }
        let item = Item {
            pid: self.next,
            name,
            added: now,
            level: 0,
        };
        self.next += 1;
        item
    }

    pub(crate) fn project(&self, page: usize) -> Option<&Project> {
        match self.pages.get(page)? {
            Page::Project(project) => Some(project),
            _ => None,
        }
    }

    pub(crate) fn path(&self) -> Vec<usize> {
        let mut path = vec![self.page];
        let mut project = self.project(self.page);
        while let Some(child) = project.and_then(Project::selected) {
            path.push(project.map_or(0, |project| project.page - 1));
            project = Some(child);
        }
        path
    }

    pub(crate) fn project_at(&self, path: &[usize]) -> Option<&Project> {
        let (&first, rest) = path.split_first()?;
        rest.iter()
            .try_fold(self.project(first)?, |project, &index| {
                project.projects.get(index)
            })
    }

    pub(crate) fn project_at_mut(&mut self, path: &[usize]) -> Option<&mut Project> {
        let (&first, rest) = path.split_first()?;
        let Page::Project(top) = self.pages.get_mut(first)? else {
            return None;
        };
        rest.iter().try_fold(&mut **top, |project, &index| {
            project.projects.get_mut(index)
        })
    }

    pub(crate) fn active(&self) -> Option<&Project> {
        self.project_at(&self.path())
    }

    pub(crate) fn active_mut(&mut self) -> Option<&mut Project> {
        let path = self.path();
        self.project_at_mut(&path)
    }

    pub(crate) fn detach(&mut self, path: &[usize]) -> Option<Project> {
        let (&last, parent) = path.split_last()?;
        if parent.is_empty() {
            self.project(last)?;
            let Page::Project(project) = self.pages.remove(last) else {
                return None;
            };
            self.page = self.page.min(self.pages.len() - 1);
            return Some(*project);
        }
        let parent = self.project_at_mut(parent)?;
        if last >= parent.projects.len() {
            return None;
        }
        parent.page = match parent.page.checked_sub(1) {
            Some(child) if child == last => 0,
            Some(child) if child > last => child,
            _ => parent.page,
        };
        Some(parent.projects.remove(last))
    }

    pub(crate) fn attach(
        &mut self,
        parent: &[usize],
        index: usize,
        project: Project,
    ) -> Option<Vec<usize>> {
        let index = if parent.is_empty() {
            let index = index.min(self.pages.len());
            self.pages.insert(index, Page::Project(Box::new(project)));
            index
        } else {
            let parent = self.project_at_mut(parent)?;
            let index = index.min(parent.projects.len());
            if parent.page > index {
                parent.page += 1;
            }
            parent.projects.insert(index, project);
            index
        };
        let mut path = parent.to_vec();
        path.push(index);
        Some(path)
    }

    pub(crate) fn has(&self, command: Command) -> bool {
        self.options.contains(&command)
    }

    pub(crate) fn remember(&mut self, text: &str) {
        self.recent.retain(|entry| entry != text);
        self.recent.insert(0, text.to_owned());
        self.recent.truncate(26);
    }

    pub(crate) fn snapshot(&self) -> String {
        let content = (&self.inbox, self.next, &self.list, &self.pages, self.page);
        serde_json::to_string(&content).unwrap_or_default()
    }

    pub(crate) fn restore(&mut self, snapshot: &str) {
        if let Ok((inbox, next, list, pages, page)) = serde_json::from_str(snapshot) {
            (self.inbox, self.next, self.list, self.pages, self.page) =
                (inbox, next, list, pages, page);
        }
    }
}

impl Project {
    pub(crate) fn repair(&mut self) {
        let mut seen = BTreeSet::new();
        if !self.tasks.iter().all(|task| seen.insert(task.id)) {
            for (id, task) in (0..).zip(&mut self.tasks) {
                task.id = id;
            }
        }
        tidy(&mut self.tasks);
        self.page = self.page.min(self.projects.len());
        for project in &mut self.projects {
            project.repair();
        }
    }

    pub(crate) fn selected(&self) -> Option<&Self> {
        self.projects.get(self.page.checked_sub(1)?)
    }

    pub(crate) fn fresh(&self) -> u32 {
        self.tasks.iter().map(|task| task.id + 1).max().unwrap_or(0)
    }

    pub(crate) fn index(&self, id: u32) -> Option<usize> {
        self.tasks.iter().position(|task| task.id == id)
    }

    pub(crate) fn tasks(&self) -> Box<dyn Iterator<Item = &Task> + '_> {
        Box::new(
            self.tasks
                .iter()
                .chain(self.projects.iter().flat_map(Self::tasks)),
        )
    }
}

impl Page {
    pub(crate) fn title(&self) -> &str {
        match self {
            Self::Inbox => "Inbox",
            Self::Performance => "Performance",
            Self::Project(project) => &project.name,
        }
    }
}

impl Node for Item {
    fn key(&self) -> u32 {
        self.pid
    }

    fn level(&self) -> usize {
        self.level
    }

    fn set_level(&mut self, level: usize) {
        self.level = level;
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl Node for Task {
    fn key(&self) -> u32 {
        self.id
    }

    fn level(&self) -> usize {
        self.level
    }

    fn set_level(&mut self, level: usize) {
        self.level = level;
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub(crate) fn tidy<T: Node>(nodes: &mut [T]) {
    let mut ceiling = 0;
    for node in nodes {
        node.set_level(node.level().min(ceiling));
        ceiling = node.level() + 1;
    }
}

impl Status {
    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Running => "Running",
            Self::NotResponding => "Not Responding",
        }
    }

    #[must_use]
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Stopped => Self::Running,
            Self::Running => Self::NotResponding,
            Self::NotResponding => Self::Stopped,
        }
    }
}

fn lenient<'de, D, T, C>(deserializer: D) -> Result<C, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
    C: FromIterator<T>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
}

mod samples {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    use super::Sample;

    pub(super) fn serialize<S: Serializer>(
        samples: &[Sample],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let text: Vec<String> = samples
            .iter()
            .map(|sample| format!("{}/{}/{}", sample.cpu, sample.kernel, sample.memory))
            .collect();
        serializer.serialize_str(&text.join(" "))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<Sample>, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.split_whitespace()
            .map(|sample| {
                let mut parts = sample.split('/').map(str::parse::<u64>);
                match (parts.next(), parts.next(), parts.next(), parts.next()) {
                    (Some(Ok(cpu)), Some(Ok(kernel)), Some(Ok(memory)), None) => Ok(Sample {
                        cpu: u8::try_from(cpu.min(100)).unwrap_or(100),
                        kernel: u8::try_from(kernel.min(100)).unwrap_or(100),
                        memory,
                    }),
                    _ => Err(D::Error::custom(format!("malformed sample `{sample}`"))),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> Timestamp {
        Timestamp::from_second(1_800_000_000).unwrap()
    }

    #[test]
    fn a_fresh_state_has_one_workspace_with_an_inbox_and_performance() {
        let mut state = State::default();
        state.repair(now());
        let workspace = state.current();
        assert_eq!(workspace.name, FIRST);
        assert!(matches!(
            workspace.pages[..],
            [Page::Inbox, Page::Performance]
        ));
        assert_eq!(workspace.next, SYSTEM + 1);
    }

    #[test]
    fn hand_edited_files_are_repaired_without_losing_anything() {
        let json = r#"{"workspace": 7, "workspaces": [{"name": "Work",
            "inbox": [{"pid": 40, "name": "Call Anna", "added": "2027-01-15T08:00:00Z"}],
            "pages": [{"project": {"name": "Home"}}, "performance", "performance"], "page": 9}]}"#;
        let mut state: State = serde_json::from_str(json).unwrap();
        state.repair(now());
        let workspace = state.current();
        assert_eq!(state.workspace, 0);
        assert_eq!(workspace.next, 41);
        let titles: Vec<&str> = workspace.pages.iter().map(Page::title).collect();
        assert_eq!(titles, ["Inbox", "Home", "Performance"]);
        assert_eq!(workspace.page, 2);
    }

    #[test]
    fn options_from_older_versions_are_dropped_or_moved_to_workspaces() {
        let json = r#"{"options": ["always_on_top", "minimize_on_use", {"icons": "details"},
            "hide_system_processes", {"speed": "low"}], "workspaces": [{"name": "Work"}]}"#;
        let mut state: State = serde_json::from_str(json).unwrap();
        state.repair(now());
        assert_eq!(state.options, BTreeSet::from([Command::AlwaysOnTop]));
        assert_eq!(
            state.current().options,
            BTreeSet::from([Command::HideSystemProcesses, Command::Speed(Speed::Low)])
        );
        assert_eq!(state.current().columns, metric::DEFAULT);
    }

    #[test]
    fn subprojects_nest_and_remember_their_tab() {
        let json = r#"{"workspaces": [{"pages": ["inbox", {"project": {"name": "Home", "page": 2,
            "projects": [{"name": "Kitchen"}, {"name": "Garden", "page": 1,
            "projects": [{"name": "Shed", "tasks": [{"name": "Paint"}]}]}]}}], "page": 1}]}"#;
        let mut state: State = serde_json::from_str(json).unwrap();
        state.repair(now());
        let workspace = state.current();
        assert_eq!(workspace.path(), [1, 1, 0]);
        assert_eq!(workspace.active().unwrap().name, "Shed");
        let home = workspace.project(1).unwrap();
        assert_eq!(
            home.tasks()
                .map(|task| task.name.as_str())
                .collect::<Vec<_>>(),
            ["Paint"]
        );
    }

    #[test]
    fn tasks_get_ids_and_levels_that_make_a_tree() {
        let json = r#"{"name": "Home", "tasks": [{"name": "a", "level": 2}, {"name": "b", "level": 3},
            {"name": "c", "level": 1}]}"#;
        let mut project: Project = serde_json::from_str(json).unwrap();
        project.repair();
        let shape: Vec<(u32, usize)> = project
            .tasks
            .iter()
            .map(|task| (task.id, task.level))
            .collect();
        assert_eq!(shape, [(0, 0), (1, 1), (2, 1)]);
        assert_eq!(project.fresh(), 3);
    }

    #[test]
    fn history_fits_on_one_line() {
        let mut workspace = Workspace::new("Work".to_owned(), now());
        workspace.history.samples = vec![
            Sample {
                cpu: 3,
                kernel: 1,
                memory: 21_628,
            };
            2
        ];
        let json = serde_json::to_string(&workspace.history).unwrap();
        assert!(json.contains(r#""samples":"3/1/21628 3/1/21628""#));
        let back: History = serde_json::from_str(&json).unwrap();
        assert_eq!(back.samples, workspace.history.samples);
        assert!(serde_json::from_str::<History>(r#"{"samples": "1/2"}"#).is_err());
    }

    #[test]
    fn workspaces_have_their_own_pids_and_remember_recent_entries() {
        let mut workspace = Workspace::new("Work".to_owned(), now());
        assert_eq!(workspace.add("A".to_owned(), now(), 4242).pid, 9);
        assert_eq!(workspace.add("B".to_owned(), now(), 10).pid, 11);
        for text in ["one", "two", "one"] {
            workspace.remember(text);
        }
        assert_eq!(workspace.recent, ["one", "two"]);
    }

    #[test]
    fn default_shortcuts_switch_workspaces_by_number() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains(r#""Cmd+1":{"workspace":1}"#));
        assert!(json.contains(r#""Cmd+N":"new_project""#));
        assert!(json.contains(r#""toggle":"Cmd+Alt+Shift+]""#));
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.shortcuts, settings.shortcuts);
    }
}
