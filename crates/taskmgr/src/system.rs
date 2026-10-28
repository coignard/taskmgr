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

use std::collections::HashMap;

use fastrand::Rng;
use jiff::{Timestamp, Zoned};

use crate::state::{Item, Page, SYSTEM, Sample, Status, Task, Workspace};

pub(crate) const PAGE: u64 = 64;
const WEEK: f64 = 604_800.0;
const DOUBLINGS: f64 = 10.0;
const PRESSURE: f64 = 16_384.0;
const SLOT: i8 = 4;
const SHARES: [u8; 6] = [0, 0, 1, 1, 1, 2];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Process<'a> {
    pub(crate) name: &'a str,
    pub(crate) pid: u32,
    pub(crate) cpu: u8,
    pub(crate) seconds: i64,
    pub(crate) memory: u64,
    pub(crate) delta: u64,
    pub(crate) peak: u64,
    pub(crate) handles: u64,
    pub(crate) threads: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Totals {
    pub(crate) handles: u64,
    pub(crate) threads: u64,
    pub(crate) processes: u64,
    pub(crate) commit: u64,
    pub(crate) paged: u64,
    pub(crate) nonpaged: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Load {
    cpu: HashMap<u32, u8>,
    own: u8,
    system: u8,
    pub(crate) usage: u8,
    pub(crate) kernel: u8,
    spent: [f64; 3],
    before: Option<Timestamp>,
    at: Option<Timestamp>,
}

impl Load {
    pub(crate) fn sample(&mut self, workspace: &Workspace, now: Timestamp, rng: &mut Rng) {
        let own = SHARES[rng.usize(..SHARES.len())];
        let system = u8::from(rng.u8(..16) == 0);
        let room = f64::from(100 - own - system);
        let mut jitter = || 0.75 + rng.f64() / 2.0;
        let wants: Vec<(u32, f64)> = workspace
            .inbox
            .iter()
            .map(|item| (item.pid, demand(item, now) * jitter()))
            .collect();
        let hidden: f64 = tasks(workspace).map(|task| busy(task) * jitter()).sum();
        let total = wants.iter().map(|&(_, want)| want).sum::<f64>() + hidden;
        let scale = if total > room { room / total } else { 1.0 };
        self.cpu = wants
            .into_iter()
            .map(|(pid, want)| (pid, percent(want * scale)))
            .collect();
        let hidden = percent(hidden * scale);
        let busy = self
            .cpu
            .values()
            .map(|&share| u32::from(share))
            .sum::<u32>()
            + u32::from(own + system + hidden);
        self.usage = u8::try_from(busy.min(100)).unwrap_or(100);
        self.kernel = (own + system + hidden / 2).min(self.usage);
        self.own = own;
        self.system = system;
        self.before = self.at.or(Some(now));
        self.at = Some(now);
    }

    pub(crate) fn run(&mut self, seconds: f64) {
        let shares = [100 - self.usage, self.system, self.own];
        for (spent, share) in self.spent.iter_mut().zip(shares) {
            *spent += seconds * f64::from(share) / 100.0;
        }
    }

    pub(crate) fn cpu(&self, pid: u32) -> u8 {
        self.cpu.get(&pid).copied().unwrap_or_default()
    }

    #[expect(clippy::cast_possible_truncation)]
    fn spent(&self, index: usize) -> i64 {
        self.spent[index] as i64
    }
}

pub(crate) fn processes<'a>(
    workspace: &'a Workspace,
    load: &Load,
    now: Timestamp,
    host: u32,
) -> Vec<Process<'a>> {
    let before = load.before.unwrap_or(now);
    let mut processes = vec![
        Process {
            cpu: 100 - load.usage,
            seconds: load.spent(0),
            ..idle()
        },
        Process {
            cpu: load.system,
            seconds: load.spent(1),
            ..system()
        },
    ];
    processes.extend(workspace.inbox.iter().map(|item| Process {
        cpu: load.cpu(item.pid),
        ..process(&item.name, item.pid, item.added, before, now)
    }));
    processes.push(Process {
        pid: own(workspace, host),
        cpu: load.own,
        seconds: load.spent(2),
        ..taskmgr()
    });
    processes
}

pub(crate) fn own(workspace: &Workspace, host: u32) -> u32 {
    let taken = |pid: u32| pid <= SYSTEM || workspace.inbox.iter().any(|item| item.pid == pid);
    (host..=u32::MAX).find(|&pid| !taken(pid)).unwrap_or(host)
}

pub(crate) fn totals(workspace: &Workspace, now: Timestamp) -> Totals {
    let mut census = vec![idle(), system(), taskmgr()];
    census.extend(
        workspace
            .inbox
            .iter()
            .filter(|item| item.added <= now)
            .map(|item| process(&item.name, item.pid, item.added, now, now)),
    );
    let running = tasks(workspace).filter(|task| task.status != Status::Stopped);
    census.extend(running.map(|task| process(&task.name, 0, now, now, now)));
    let stored: u64 = tasks(workspace)
        .filter(|task| task.status == Status::Stopped)
        .map(|task| characters(&task.name) * PAGE)
        .sum();
    let handles = census.iter().map(|process| process.handles).sum::<u64>();
    let threads = census.iter().map(|process| process.threads).sum::<u64>();
    let paged = handles * 8;
    let nonpaged = threads * 12;
    let memory = census.iter().map(|process| process.memory).sum::<u64>();
    Totals {
        handles,
        threads,
        processes: census.len() as u64,
        commit: paged + nonpaged + memory + stored,
        paged,
        nonpaged,
    }
}

pub(crate) fn commit(workspace: &Workspace, now: Timestamp) -> u64 {
    totals(workspace, now).commit
}

pub(crate) fn cost(name: &str) -> u64 {
    characters(name) * PAGE
}

fn tasks(workspace: &Workspace) -> impl Iterator<Item = &Task> {
    workspace
        .pages
        .iter()
        .filter_map(|page| match page {
            Page::Project(project) => Some(project.tasks()),
            _ => None,
        })
        .flatten()
}

fn process(
    name: &str,
    pid: u32,
    added: Timestamp,
    before: Timestamp,
    now: Timestamp,
) -> Process<'_> {
    let characters = characters(name);
    let words = name.split_whitespace().count().max(1) as u64;
    let minutes = |at: Timestamp| u64::try_from(seconds(added, at) / 60).unwrap_or_default();
    let (then, minutes) = (minutes(before.max(added)), minutes(now));
    let memory = characters * PAGE + minutes;
    Process {
        name,
        pid,
        cpu: 0,
        seconds: seconds(added, now),
        memory,
        delta: minutes - then.min(minutes),
        peak: memory,
        handles: characters,
        threads: words,
    }
}

const fn idle() -> Process<'static> {
    Process {
        name: "System Idle Process",
        pid: 0,
        cpu: 0,
        seconds: 0,
        memory: 16,
        delta: 0,
        peak: 16,
        handles: 0,
        threads: 1,
    }
}

const fn system() -> Process<'static> {
    Process {
        name: "System",
        pid: SYSTEM,
        memory: 212,
        peak: 1_256,
        handles: 220,
        threads: 40,
        ..idle()
    }
}

const fn taskmgr() -> Process<'static> {
    Process {
        name: "taskmgr.exe",
        memory: 2_096,
        peak: 2_104,
        handles: 30,
        threads: 3,
        ..idle()
    }
}

pub(crate) fn record(workspace: &mut Workspace, now: &Zoned) -> bool {
    let zone = now.time_zone();
    let start = workspace.history.until;
    let mut until =
        start.unwrap_or_else(|| slot(&workspace.created.to_zoned(zone.clone())).timestamp());
    loop {
        let boundary = next(&until.to_zoned(zone.clone())).timestamp();
        if boundary > now.timestamp() {
            break;
        }
        let busy: f64 = workspace
            .inbox
            .iter()
            .filter(|item| item.added <= boundary)
            .map(|item| demand(item, boundary))
            .chain(tasks(workspace).map(busy))
            .sum();
        let memory = commit(workspace, boundary);
        workspace.peak = workspace.peak.max(memory);
        workspace.history.samples.push(Sample {
            cpu: percent((busy + 1.0).min(100.0)),
            kernel: 1,
            memory,
        });
        until = boundary;
    }
    workspace.history.until = Some(until);
    start != Some(until)
}

pub(crate) fn midnight(workspace: &Workspace, zone: &jiff::tz::TimeZone) -> i32 {
    workspace.history.until.map_or(0, |until| {
        i32::from(until.to_zoned(zone.clone()).hour() / SLOT)
    })
}

fn slot(time: &Zoned) -> Zoned {
    let start = time.date().at(time.hour() / SLOT * SLOT, 0, 0, 0);
    start
        .to_zoned(time.time_zone().clone())
        .unwrap_or_else(|_| time.clone())
}

fn next(time: &Zoned) -> Zoned {
    let start = slot(time);
    let hour = start.hour() + SLOT;
    let civil = if hour >= 24 {
        start.date().tomorrow().map(|date| date.at(0, 0, 0, 0))
    } else {
        Ok(start.date().at(hour, 0, 0, 0))
    };
    civil
        .and_then(|civil| civil.to_zoned(time.time_zone().clone()))
        .ok()
        .filter(|next| next > time)
        .unwrap_or_else(|| time.clone() + jiff::SignedDuration::from_hours(i64::from(SLOT)))
}

pub(crate) fn memory(item: &Item, now: Timestamp) -> u64 {
    let minutes = u64::try_from(seconds(item.added, now) / 60).unwrap_or_default();
    characters(&item.name) * PAGE + minutes
}

fn busy(task: &Task) -> f64 {
    #[expect(clippy::cast_precision_loss)]
    let weight = 1.0 + characters(&task.name) as f64 / 8.0;
    match task.status {
        Status::Stopped => 0.0,
        Status::Running => weight,
        Status::NotResponding => 2.0 * weight,
    }
}

fn demand(item: &Item, now: Timestamp) -> f64 {
    #[expect(clippy::cast_precision_loss)]
    let (age, pressure) = (
        seconds(item.added, now) as f64,
        1.0 + memory(item, now) as f64 / PRESSURE,
    );
    let progress = (age / WEEK * pressure).min(1.0);
    100.0 * ((DOUBLINGS * progress).exp2() - 1.0) / (DOUBLINGS.exp2() - 1.0)
}

pub(crate) fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

pub(crate) fn clock(seconds: i64) -> String {
    format!(
        "{}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn seconds(from: Timestamp, to: Timestamp) -> i64 {
    (to.as_second() - from.as_second()).max(0)
}

fn characters(text: &str) -> u64 {
    text.chars().count() as u64
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn percent(value: f64) -> u8 {
    value.clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use jiff::SignedDuration;

    use super::*;
    use crate::state::Project;

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    fn inbox(added: &Zoned, names: &[&str]) -> Workspace {
        let mut workspace = Workspace::new("Work".to_owned(), added.timestamp());
        for name in names {
            let item = workspace.add((*name).to_owned(), added.timestamp(), 4242);
            workspace.inbox.push(item);
        }
        workspace
    }

    #[test]
    fn items_start_idle_and_saturate_the_processor_within_a_week() {
        let start = at("2026-10-27T09:00:00+01:00[Europe/Paris]");
        let workspace = inbox(&start, &["Call the bank about the card"]);
        let item = &workspace.inbox[0];
        let after = |hours: i64| start.timestamp() + SignedDuration::from_hours(hours);
        assert_eq!(demand(item, after(0)), 0.0);
        assert!(demand(item, after(24)) < 1.0);
        assert!(demand(item, after(24 * 4)) > 10.0 * demand(item, after(24 * 2)));
        assert!((demand(item, after(24 * 7)) - 100.0).abs() < f64::EPSILON);
        assert_eq!(memory(item, after(0)), 28 * 64);
        assert_eq!(memory(item, after(2)), 28 * 64 + 120);
    }

    #[test]
    fn samples_share_one_processor() {
        let start = at("2026-10-01T09:00:00Z[UTC]");
        let workspace = inbox(&start, &["one", "two", "three"]);
        let now = start.timestamp() + SignedDuration::from_hours(24 * 30);
        let mut rng = Rng::with_seed(7);
        let mut load = Load::default();
        for _ in 0..50 {
            load.sample(&workspace, now, &mut rng);
            let total: u32 = processes(&workspace, &load, now, 4242)
                .iter()
                .map(|process| u32::from(process.cpu))
                .sum();
            assert_eq!(total, 100);
            assert!(load.usage >= 90);
        }
    }

    #[test]
    fn running_project_tasks_load_the_processor_out_of_sight() {
        let start = at("2026-10-01T09:00:00Z[UTC]");
        let mut workspace = inbox(&start, &[]);
        let tasks = (0..20)
            .map(|id| Task {
                id,
                name: "Rewrite the importer".to_owned(),
                status: if id % 2 == 0 {
                    Status::Running
                } else {
                    Status::NotResponding
                },
                level: 0,
            })
            .collect();
        workspace.pages.insert(
            1,
            Page::Project(Box::new(Project {
                tasks,
                ..Project::default()
            })),
        );
        let mut load = Load::default();
        load.sample(&workspace, start.timestamp(), &mut Rng::with_seed(7));
        assert!(load.usage > 90);
        let shown = processes(&workspace, &load, start.timestamp(), 4242);
        assert_eq!(shown.len(), 3);
        assert_eq!(shown[0].cpu, 100 - load.usage);
        let totals = totals(&workspace, start.timestamp());
        assert_eq!(totals.processes, 23);
        assert_eq!(
            totals.commit,
            totals.paged + totals.nonpaged + 16 + 212 + 2_096 + 20 * 20 * 64
        );
    }

    #[test]
    fn system_processes_count_cpu_time_only_while_running() {
        let start = at("2026-10-27T09:00:00Z[UTC]");
        let workspace = inbox(&start, &[]);
        let mut load = Load::default();
        load.sample(&workspace, start.timestamp(), &mut Rng::with_seed(3));
        let idle = 100 - load.usage;
        load.run(100.0);
        let shown = processes(&workspace, &load, start.timestamp(), 4242);
        assert_eq!(shown[0].seconds, i64::from(idle));
    }

    #[test]
    fn taskmgr_never_borrows_an_inbox_pid() {
        let start = at("2026-10-27T09:00:00Z[UTC]");
        let mut workspace = inbox(&start, &["a", "b"]);
        assert_eq!(own(&workspace, 10), 11);
        assert_eq!(own(&workspace, 3), 11);
        assert_eq!(own(&workspace, 4242), 4242);
        workspace.next = 4242;
        assert_eq!(
            workspace.add("c".to_owned(), start.timestamp(), 4242).pid,
            4243
        );
    }

    #[test]
    fn processes_come_in_pid_order_around_the_inbox() {
        let start = at("2026-10-27T09:00:00Z[UTC]");
        let workspace = inbox(&start, &["Plan the trip"]);
        let names: Vec<(&str, u32)> =
            processes(&workspace, &Load::default(), start.timestamp(), 4242)
                .iter()
                .map(|process| (process.name, process.pid))
                .collect();
        assert_eq!(
            names,
            [
                ("System Idle Process", 0),
                ("System", 8),
                ("Plan the trip", 9),
                ("taskmgr.exe", 4242)
            ]
        );
    }

    #[test]
    fn history_keeps_six_samples_a_day_on_local_boundaries() {
        let start = at("2026-10-27T09:30:00+01:00[Europe/Paris]");
        let mut workspace = inbox(&start, &["Write the report"]);
        assert!(record(&mut workspace, &start));
        assert_eq!(midnight(&workspace, start.time_zone()), 2);
        let later = at("2026-10-28T01:00:00+01:00[Europe/Paris]");
        assert!(record(&mut workspace, &later));
        assert_eq!(workspace.history.samples.len(), 4);
        assert_eq!(midnight(&workspace, later.time_zone()), 0);
        assert!(!record(&mut workspace, &later));
        let memory = workspace.history.samples[3].memory;
        assert_eq!(memory, commit(&workspace, later.timestamp()) - 60);
    }
}
