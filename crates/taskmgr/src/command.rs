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

use serde::{Deserialize, Serialize};

use crate::menu::{Entry, Menu};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Command {
    NewProject,
    NewSubproject,
    RenameProject,
    DeleteProject,
    NewWorkspace,
    RenameWorkspace,
    DeleteWorkspace,
    Workspace(usize),
    Exit,
    AlwaysOnTop,
    HideWhenMinimized,
    HideDockIcon,
    HideSystemProcesses,
    Refresh,
    Speed(Speed),
    SelectColumns,
    GraphPerProcessor,
    KernelTimes,
    About,
    EndTask,
    SwitchTo,
    NewTask,
    EndProcess,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Speed {
    High,
    Normal,
    Low,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Project,
    Inbox,
    Performance,
}

impl Kind {
    pub(crate) const fn buttons(self) -> (i32, &'static [(&'static str, Command)]) {
        match self {
            Self::Project => (
                80,
                &[
                    ("&End Task", Command::EndTask),
                    ("&Switch To", Command::SwitchTo),
                    ("&New Task...", Command::NewTask),
                ],
            ),
            Self::Inbox => (99, &[("&End Process", Command::EndProcess)]),
            Self::Performance => (0, &[]),
        }
    }
}

pub(crate) fn bar(
    kind: Kind,
    nested: bool,
    workspaces: &[&str],
    shortcut: impl Fn(Command) -> String,
) -> Vec<Menu> {
    let item = |label: &str, command| {
        let label = format!("{label}\t{}", shortcut(command));
        Entry::Item(label.trim_end().to_owned(), command)
    };
    let speed = Entry::Submenu(
        "&Update Speed".to_owned(),
        vec![
            item("&High", Command::Speed(Speed::High)),
            item("&Normal", Command::Speed(Speed::Normal)),
            item("&Low", Command::Speed(Speed::Low)),
            item("&Paused", Command::Speed(Speed::Paused)),
        ],
    );
    let mut switch = vec![
        item("&New Workspace...", Command::NewWorkspace),
        item("&Rename Workspace...", Command::RenameWorkspace),
        item("&Delete Workspace", Command::DeleteWorkspace),
    ];
    if workspaces.len() > 1 {
        switch.push(Entry::Separator);
        switch.extend(workspaces.iter().zip(1..).map(|(name, ordinal)| {
            let label = format!("&{} {}", ordinal % 10, name.replace('&', "&&"));
            item(&label, Command::Workspace(ordinal))
        }));
    }
    let mut view = vec![
        item("&Refresh Now", Command::Refresh),
        speed,
        Entry::Separator,
    ];
    match kind {
        Kind::Project => {}
        Kind::Inbox => view.extend([
            item("&Select Columns...", Command::SelectColumns),
            Entry::Separator,
        ]),
        Kind::Performance => view.extend([
            Entry::Submenu(
                "&CPU History".to_owned(),
                vec![item("One Graph &Per CPU", Command::GraphPerProcessor)],
            ),
            item("&Show Kernel Times", Command::KernelTimes),
            Entry::Separator,
        ]),
    }
    view.push(Entry::Submenu("&Workspaces".to_owned(), switch));
    vec![
        Menu {
            title: "&File",
            entries: vec![
                Entry::Submenu(
                    "&New".to_owned(),
                    vec![
                        item("&Project...", Command::NewProject),
                        item("&Subproject...", Command::NewSubproject),
                    ],
                ),
                if nested {
                    item("&Rename Subproject...", Command::RenameProject)
                } else {
                    item("&Rename Project...", Command::RenameProject)
                },
                if nested {
                    item("&Delete Subproject", Command::DeleteProject)
                } else {
                    item("&Delete Project", Command::DeleteProject)
                },
                Entry::Separator,
                item("E&xit Task Manager", Command::Exit),
            ],
        },
        Menu {
            title: "&Options",
            entries: vec![
                item("&Always On Top", Command::AlwaysOnTop),
                item("&Hide When Minimized", Command::HideWhenMinimized),
                item("Hide &Dock Icon", Command::HideDockIcon),
                item("Hide &System Processes", Command::HideSystemProcesses),
            ],
        },
        Menu {
            title: "&View",
            entries: view,
        },
        Menu {
            title: "&Help",
            entries: vec![item("&About Task Manager", Command::About)],
        },
    ]
}

impl Command {
    pub(crate) fn help(self) -> &'static str {
        match self {
            Self::NewProject => "Creates a new project",
            Self::NewSubproject => "Creates a new project inside the current one",
            Self::RenameProject => "Renames the current project",
            Self::DeleteProject => {
                "Deletes the current project with all of its tasks and subprojects"
            }
            Self::NewWorkspace => "Creates a new workspace with its own inbox and projects",
            Self::RenameWorkspace => "Renames the current workspace",
            Self::DeleteWorkspace => {
                "Deletes the current workspace with its inbox, projects and history"
            }
            Self::Workspace(_) => "Switches to this workspace",
            Self::Exit => "Exits the Task Manager application",
            Self::AlwaysOnTop => {
                "Task Manager remains in front of all other windows unless minimized"
            }
            Self::HideWhenMinimized => "Hides the Task Manager when it is minimized",
            Self::HideDockIcon => "Hides the Task Manager icon in the Dock",
            Self::HideSystemProcesses => {
                "Hides System Idle Process, System and taskmgr.exe on the Inbox page"
            }
            Self::Refresh => "Force Task Manager to update now, regardless of Update Speed setting",
            Self::Speed(Speed::High) => "Updates the display twice per second",
            Self::Speed(Speed::Normal) => "Updates the display every two seconds",
            Self::Speed(Speed::Low) => "Updates the display every four seconds",
            Self::Speed(Speed::Paused) => "Display does not automatically update",
            Self::SelectColumns => "Select which columns will be visible on the Process page",
            Self::GraphPerProcessor => "Each CPU has its own history graph",
            Self::KernelTimes => "Displays kernel time in the performance graphs",
            Self::About => "Displays program information, version number, and copyright",
            Self::EndTask | Self::SwitchTo | Self::NewTask | Self::EndProcess => "",
        }
    }

    pub(crate) const fn local(self) -> bool {
        matches!(
            self,
            Self::Speed(_)
                | Self::HideSystemProcesses
                | Self::GraphPerProcessor
                | Self::KernelTimes
        )
    }

    pub(crate) const fn interval(self) -> Option<i64> {
        match self {
            Self::Speed(Speed::High) => Some(500),
            Self::Speed(Speed::Normal) => Some(2000),
            Self::Speed(Speed::Low) => Some(4000),
            _ => None,
        }
    }
}
