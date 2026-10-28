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

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use jiff::Timestamp;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tempfile::NamedTempFile;

use crate::state::{Settings, State};

const STATE: &str = "taskmgr.json";
const SETTINGS: &str = "settings.json";

#[derive(Debug)]
pub struct Storage {
    directory: PathBuf,
}

impl Storage {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    pub(crate) fn settings(&self, now: Timestamp) -> Settings {
        let (settings, changed) = match self.read::<Settings>(SETTINGS, now) {
            Some(mut settings) => {
                let changed = settings.adopt(Settings::default());
                (settings, changed)
            }
            None => (Settings::default(), true),
        };
        if changed {
            let _ = self.write(SETTINGS, &settings);
        }
        settings
    }

    pub(crate) fn state(&self, now: Timestamp) -> State {
        self.read(STATE, now).unwrap_or_default()
    }

    pub(crate) fn save(&self, state: &State) -> io::Result<()> {
        self.write(STATE, state)
    }

    fn read<T: DeserializeOwned>(&self, name: &str, now: Timestamp) -> Option<T> {
        let path = self.directory.join(name);
        let text = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&text)
            .inspect_err(|_| {
                let broken = format!("{name}.broken-{}", now.as_second());
                let _ = fs::rename(&path, self.directory.join(broken));
            })
            .ok()
    }

    fn write<T: Serialize>(&self, name: &str, value: &T) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
        fs::create_dir_all(&self.directory)?;
        let mut staged = NamedTempFile::new_in(&self.directory)?;
        staged.write_all(&json)?;
        staged.as_file().sync_all()?;
        staged.persist(self.directory.join(name))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn now() -> Timestamp {
        Timestamp::from_second(1_800_000_000).unwrap()
    }

    #[test]
    fn state_survives_a_round_trip() {
        let directory = TempDir::new().unwrap();
        let storage = Storage::new(directory.path().join("Task Manager"));
        let mut state = storage.state(now());
        state.repair(now());
        let item = state.current_mut().add("Ship it".to_owned(), now(), 0);
        state.current_mut().inbox.push(item);
        storage.save(&state).unwrap();
        let back = storage.state(now());
        assert_eq!(back.current().inbox, state.current().inbox);
    }

    #[test]
    fn unreadable_state_is_set_aside_instead_of_overwritten() {
        let directory = TempDir::new().unwrap();
        let storage = Storage::new(directory.path());
        fs::write(directory.path().join(STATE), "{ not json").unwrap();
        assert!(storage.state(now()).workspaces.is_empty());
        let aside = directory.path().join("taskmgr.json.broken-1800000000");
        assert_eq!(fs::read_to_string(aside).unwrap(), "{ not json");
    }

    #[test]
    fn settings_keep_your_shortcuts_and_gain_new_ones() {
        let directory = TempDir::new().unwrap();
        let storage = Storage::new(directory.path());
        let defaults = storage.settings(now()).shortcuts.len();
        fs::write(
            directory.path().join(SETTINGS),
            r#"{"shortcuts": {"Cmd+Alt+N": "new_project", "Cmd+1": "about"}}"#,
        )
        .unwrap();
        let settings = storage.settings(now());
        assert_eq!(settings.shortcuts.len(), defaults);
        let written = fs::read_to_string(directory.path().join(SETTINGS)).unwrap();
        assert!(written.contains("Cmd+Alt+N") && !written.contains(r#""Cmd+N""#));
        assert!(written.contains(r#""Cmd+1": "about""#));
    }
}
