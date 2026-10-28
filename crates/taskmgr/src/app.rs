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

mod actions;
mod editing;
mod input;
mod regroup;
mod spring;
mod tabs;
#[cfg(test)]
mod tests;

use std::mem;

use fastrand::Rng;
use jiff::{SignedDuration, Timestamp, Zoned};

use crate::assets::FONT;
use crate::chrome::{self, CAPTION, Caption, FRAME, Title};
use crate::command::{self, Command, Kind};
use crate::dialog::{self, Dialog, Part};
use crate::geometry::{Point, Rect, Size};
use crate::graphics::{BLACK, Canvas, TRANSPARENT};
use crate::keys::{Key, Keystroke, Modifiers};
use crate::list::{self, Column, List, Mark, Row};
use crate::menu::tracking::Tracking;
use crate::menu::{self, Menu};
use crate::pages::{self, Focus, Strip};
use crate::performance::{self, Readings};
use crate::state::{Page, Placement, SYSTEM, Settings, State};
use crate::storage::Storage;
use crate::system::{self, Load, Process};
use crate::tag;
use crate::ui::{self, Cues, Edge, Target, Ui};
use regroup::Regroup;
use spring::Spring;

const TITLE: &str = "Task Manager";
const PARTS: [i32; 2] = [96, 216];
const BLINK: i64 = 530;
const DRIFT: i64 = 50;
const GHOST: usize = 12;
const SEARCH: i64 = 1000;
const PROJECT: [Column; 2] = [
    Column {
        title: "Task",
        width: 250,
        numeric: false,
    },
    Column {
        title: "Status",
        width: 97,
        numeric: false,
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Resize(Size),
    Placed(Rect),
    Activate(bool),
    Move(Point),
    Press {
        at: Point,
        clicks: u8,
        held: Modifiers,
    },
    Release(Point),
    Scroll {
        at: Point,
        by: Point,
    },
    KeyDown(Keystroke),
    KeyUp(Key),
    Text(String),
    Paste(String),
    Tick,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Place(Rect),
    Minimize,
    Hide,
    Zoom(bool),
    Close,
    Float(bool),
    Dock(bool),
    Copy(String),
    Beep,
    Alert,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Arrow,
    Resize(Edge),
    Split,
    No,
}

#[derive(Debug)]
pub struct Scene {
    pub window: Canvas,
    pub popups: Vec<(Rect, Canvas)>,
    pub cursor: Cursor,
}

#[derive(Clone, Debug)]
pub struct Host {
    pub user: String,
    pub pid: u32,
    pub seed: u64,
}

#[derive(Debug)]
struct Sheet {
    dialog: Dialog,
    at: Point,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Place {
    Inbox,
    Project(Vec<usize>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Landing {
    Page(Place),
    Mark(Mark),
    Into(Mark),
}

#[expect(clippy::struct_excessive_bools)]
pub struct App {
    storage: Storage,
    state: State,
    settings: Settings,
    host: Host,
    rng: Rng,
    now: Zoned,
    ran: Timestamp,
    dirty: bool,
    load: Load,
    sampled: Option<Timestamp>,
    size: Size,
    active: bool,
    cues: Cues,
    alone: bool,
    focus: Focus,
    pointer: Point,
    held: Modifiers,
    pressed: Option<Target>,
    keyed: Option<(Key, Target)>,
    deferred: Option<u32>,
    anchor: Point,
    grab: Point,
    moved: bool,
    spring: Option<Spring>,
    hovering: Option<(usize, usize, Timestamp)>,
    spun: i32,
    swiped: i32,
    alt: bool,
    search: String,
    searched: Timestamp,
    first: Vec<usize>,
    subject: Vec<usize>,
    menu: Option<Tracking>,
    sheets: Vec<Sheet>,
    blink: Timestamp,
    targets: Vec<(Rect, Target)>,
}

impl App {
    pub const SIZE: Size = Size::new(404, 447);
    const MINIMUM: Size = Size::new(320, 300);

    pub fn new(storage: Storage, host: Host, now: Zoned) -> Self {
        let stamp = now.timestamp();
        let mut state = storage.state(stamp);
        state.repair(stamp);
        let settings = storage.settings(stamp);
        let size = state
            .window
            .map_or(Self::SIZE, |placement| placement.frame.size());
        let mut app = Self {
            storage,
            state,
            settings,
            rng: Rng::with_seed(host.seed),
            host,
            now,
            ran: stamp,
            dirty: true,
            load: Load::default(),
            sampled: None,
            size,
            active: true,
            cues: Cues::default(),
            alone: false,
            focus: Focus::List,
            pointer: Point::new(-1, -1),
            held: Modifiers::default(),
            pressed: None,
            keyed: None,
            deferred: None,
            anchor: Point::default(),
            grab: Point::default(),
            moved: false,
            spring: None,
            hovering: None,
            spun: 0,
            swiped: 0,
            alt: false,
            search: String::new(),
            searched: stamp,
            first: Vec::new(),
            subject: Vec::new(),
            menu: None,
            sheets: Vec::new(),
            blink: stamp,
            targets: Vec::new(),
        };
        app.advance();
        app.persist();
        app
    }

    pub fn placement(&self) -> Option<Placement> {
        self.state.window
    }

    pub fn floats(&self) -> bool {
        self.state.options.contains(&Command::AlwaysOnTop)
    }

    pub fn docked(&self) -> bool {
        !self.state.options.contains(&Command::HideDockIcon)
    }

    pub fn hotkey(&self) -> Option<Keystroke> {
        self.settings.toggle
    }

    pub fn title(&self) -> String {
        match &self.state.workspaces[..] {
            [_] => TITLE.to_owned(),
            _ => format!("{TITLE} ({})", self.state.current().name),
        }
    }

    pub fn wake(&self) -> Option<Timestamp> {
        let now = self.now.timestamp();
        let second = Timestamp::from_second(now.as_second() + 1).ok();
        let sample = self
            .sampled
            .zip(self.interval())
            .map(|(at, every)| at + every);
        let menu = self.menu.as_ref().and_then(|menu| menu.due);
        let caret = self.blinking().then(|| {
            self.blink + SignedDuration::from_millis((self.blinked() / BLINK + 1) * BLINK)
        });
        let drift = (self.drift() != 0).then(|| now + SignedDuration::from_millis(DRIFT));
        [second, sample, menu, caret, drift, self.ripe()]
            .into_iter()
            .flatten()
            .min()
    }

    pub fn handle(&mut self, event: Event, now: Zoned) -> Option<Effect> {
        self.now = now;
        self.advance();
        let shown = self.shown();
        let effect = match event {
            Event::Resize(size) => {
                self.size = size;
                None
            }
            Event::Placed(frame) => {
                if !self.zoomed() && self.state.window.map(|window| window.frame) != Some(frame) {
                    self.state.window = Some(Placement {
                        frame,
                        zoomed: false,
                    });
                    self.dirty = true;
                }
                None
            }
            Event::Activate(active) => {
                self.active = active;
                self.alone = false;
                if !active {
                    self.menu = None;
                    self.pressed = None;
                    self.keyed = None;
                    self.moved = false;
                    self.home();
                }
                None
            }
            Event::Move(at) => {
                self.pointer = at;
                self.hover();
                self.linger();
                None
            }
            Event::Press { at, clicks, held } => {
                self.pointer = at;
                self.held = held;
                self.press(clicks)
            }
            Event::Release(at) => {
                self.pointer = at;
                self.release()
            }
            Event::Scroll { .. } if self.menu.is_some() => None,
            Event::Scroll { at, by } => {
                self.pointer = at;
                self.wheel(by);
                None
            }
            Event::KeyDown(keystroke) => self.key(keystroke),
            Event::KeyUp(key) => self.lift(key),
            Event::Text(_) | Event::Paste(_) if self.menu.is_some() || self.dragging() => None,
            Event::Text(text) => {
                if self.sheets.is_empty() {
                    self.seek(&text);
                } else {
                    self.type_in(&text, false);
                }
                None
            }
            Event::Paste(text) => {
                if self.sheets.is_empty() {
                    self.paste(&text)
                } else {
                    self.type_in(&text, true);
                    None
                }
            }
            Event::Tick => {
                self.linger();
                let now = self.now.timestamp();
                if let Some(menu) = &mut self.menu {
                    menu.tick(now);
                }
                let drift = self.drift();
                if drift != 0
                    && let Some(inside) = self.inside()
                    && let Some(list) = self.list_mut()
                {
                    list.nudge(inside, true, drift);
                }
                None
            }
        };
        if self.shown() != shown {
            self.refresh();
            if let (Some(inside), Some(list)) = (self.inside(), self.list_mut()) {
                list.reveal(inside);
            }
        }
        self.persist();
        effect
    }

    fn shown(&self) -> (usize, Vec<usize>, Option<u32>) {
        let focus = self.list().and_then(|list| list.focus);
        (self.state.workspace, self.state.current().path(), focus)
    }

    pub fn scene(&mut self) -> Scene {
        self.refresh();
        let holding = self
            .pressed
            .filter(|&target| self.hit() == Some(target))
            .or(self.keyed.map(|(_, target)| target));
        let ui = self.window(holding);
        let mut targets = ui.targets;
        let window = ui.canvas;
        let popups = self.popups(&window, holding, &mut targets);
        self.targets = targets;
        Scene {
            window,
            popups,
            cursor: self.cursor(),
        }
    }

    fn window(&self, holding: Option<Target>) -> Ui {
        let mut ui = Ui::new(self.size, holding, self.cues);
        let zoomed = self.zoomed();
        let active = self.active && self.sheets.is_empty();
        chrome::frame(&mut ui, self.size, !zoomed);
        let title = self.title();
        let bar = Caption {
            area: Rect::new(FRAME, FRAME, self.size.width - 2 * FRAME, CAPTION),
            title: &title,
            active,
            icon: true,
            drag: Target::Caption,
        };
        chrome::caption(&mut ui, &bar, &chrome::knobs(zoomed));
        let menus = self.menus();
        let hot = match self.hit() {
            Some(Target::Title(title)) if self.pressed.is_none() => Some(title),
            _ => None,
        };
        let look = |index| match &self.menu {
            Some(menu) if menu.title == index && menu.dropped() => Title::Open,
            Some(menu) if menu.title == index => Title::Hot,
            None if hot == Some(index) => Title::Hot,
            _ => Title::Plain,
        };
        let keyboard = self.menu.as_ref().is_some_and(|menu| menu.keyboard);
        chrome::menu_bar(&mut ui, &menus, look, active, keyboard);
        let focus = active.then_some(self.focus);
        let landing = self.landing();
        let panel = self.tabs(&mut ui, focus, landing.as_ref());
        let content = pages::content(panel);
        let kind = self.kind();
        if let Some(list) = self.list() {
            let (width, buttons) = kind.buttons();
            let buttons: Vec<_> = buttons
                .iter()
                .map(|&(label, command)| (label, command, self.enabled(command)))
                .collect();
            let mark = match landing {
                Some(Landing::Mark(mark) | Landing::Into(mark)) => Some(mark),
                _ => None,
            };
            list::draw(&mut ui, content, list, focus == Some(Focus::List), mark);
            pages::buttons(&mut ui, panel, focus, &buttons, width);
            if let Some(track) = self.track(self.pressed) {
                ui.canvas.invert(track);
            }
        } else {
            performance::draw(&mut ui.canvas, panel, &self.readings());
        }
        let workspace = self.state.current();
        let totals = system::totals(workspace, self.now.timestamp());
        let parts = [
            format!("Processes: {}", totals.processes),
            format!("CPU Usage: {}%", self.load.usage),
            format!("Mem Usage: {}K / {}K", totals.commit, self.settings.memory),
        ];
        let parts = parts.each_ref().map(String::as_str);
        let hint = self.menu.as_ref().map(|menu| menu.help);
        chrome::status_bar(&mut ui, &PARTS, &parts, hint, !zoomed);
        ui
    }

    fn tabs(&self, ui: &mut Ui, focus: Option<Focus>, landing: Option<&Landing>) -> Rect {
        let regroup = self.regroup();
        let dropped = match (landing, &regroup, self.hit()) {
            (Some(Landing::Page(_)), _, Some(Target::Tab(depth, index))) => Some((depth, index)),
            (_, Some(Regroup::Into { level, index, .. }), _) => Some((*level, *index)),
            _ => None,
        };
        let guest = match &regroup {
            Some(Regroup::Among {
                source, level, gap, ..
            }) => {
                let project = self.state.current().project_at(source);
                project.map(|project| (*level, *gap, project.name.as_str()))
            }
            _ => None,
        };
        let hidden = match &regroup {
            Some(Regroup::Among { source, .. }) => source.split_last(),
            _ => None,
        };
        let rows: Vec<(Vec<String>, Option<usize>)> =
            (0..self.depth()).map(|depth| self.titles(depth)).collect();
        let count = hidden.map_or(rows.len(), |(_, parent)| {
            parent.len() + usize::from(rows[parent.len()].0.len() > 1)
        });
        let titles: Vec<Vec<&str>> = rows
            .iter()
            .map(|(titles, _)| titles.iter().map(String::as_str).collect())
            .collect();
        let strips: Vec<Strip> = titles
            .iter()
            .zip(&rows)
            .take(count)
            .enumerate()
            .map(|(depth, (titles, &(_, selected)))| Strip {
                depth,
                titles,
                selected,
                first: self.first.get(depth).copied().unwrap_or(0),
                focused: focus == Some(Focus::Tabs(depth)),
                drop: dropped
                    .filter(|&(level, _)| level == depth)
                    .map(|(_, index)| index),
                guest: guest
                    .filter(|&(level, ..)| level == depth)
                    .map(|(_, gap, name)| (gap, name)),
                absent: hidden
                    .filter(|(_, parent)| parent.len() == depth)
                    .map(|(&index, _)| index),
            })
            .collect();
        pages::tabs(ui, self.strip(), &strips)
    }

    fn popups(
        &mut self,
        window: &Canvas,
        holding: Option<Target>,
        targets: &mut Vec<(Rect, Target)>,
    ) -> Vec<(Rect, Canvas)> {
        let keyboard = self.menu.as_ref().is_some_and(|menu| menu.keyboard);
        let mut popups = Vec::new();
        if let Some(menu) = &self.menu {
            for (level, (frame, entries, highlight)) in (0..).zip(menu.chain()) {
                let mut ui = Ui::new(frame.size(), None, self.cues);
                ui.origin = Point::new(frame.x, frame.y);
                menu::draw(&mut ui, level, entries, highlight, keyboard, |command| {
                    (self.mark(command), self.enabled(command))
                });
                targets.extend(ui.targets);
                popups.push((frame, ui.canvas));
            }
        }
        let caret = self.caret();
        let cues = self.cues;
        let count = self.sheets.len();
        for (index, sheet) in self.sheets.iter_mut().enumerate() {
            let top = index + 1 == count;
            let size = sheet.dialog.size();
            let mut ui = Ui::new(size, holding.filter(|_| top), cues);
            ui.origin = sheet.at;
            dialog::draw(&mut ui, &mut sheet.dialog, self.active && top, caret && top);
            if top {
                targets.extend(ui.targets);
            }
            let frame = Rect::new(sheet.at.x, sheet.at.y, size.width, size.height);
            popups.push((frame, ui.canvas));
            if let (Some(area), Dialog::Prompt(prompt), true) =
                (sheet.dialog.dropdown(sheet.at), &sheet.dialog, top)
            {
                let mut ui = Ui::new(area.size(), None, cues);
                ui.origin = Point::new(area.x, area.y);
                dialog::draw_dropdown(&mut ui, prompt);
                targets.extend(ui.targets);
                popups.push((area, ui.canvas));
            }
        }
        if let Some(ghost) = self.ghost() {
            popups.push(ghost);
        }
        if let Some((area, thickness)) = self.outline() {
            let mut layers = vec![(Point::default(), window)];
            layers.extend(
                popups
                    .iter()
                    .map(|(frame, canvas)| (Point::new(frame.x, frame.y), canvas)),
            );
            let outline = chrome::outline(&layers, area, thickness);
            popups.push((area, outline));
        }
        popups
    }

    fn advance(&mut self) {
        let now = self.now.clone();
        let stamp = now.timestamp();
        let elapsed = stamp.duration_since(mem::replace(&mut self.ran, stamp));
        self.load.run(elapsed.as_secs_f64().max(0.0));
        if system::record(self.state.current_mut(), &now) {
            self.dirty = true;
        }
        let due = self.sampled.is_none_or(|at| {
            self.interval()
                .is_some_and(|every| stamp < at || stamp >= at + every)
        });
        if due {
            self.load.sample(self.state.current(), stamp, &mut self.rng);
            self.sampled = Some(stamp);
        }
        let commit = system::commit(self.state.current(), stamp);
        let workspace = self.state.current_mut();
        if commit > workspace.peak {
            workspace.peak = commit;
            self.dirty = true;
        }
    }

    fn persist(&mut self) {
        if self.dirty && self.storage.save(&self.state).is_ok() {
            self.dirty = false;
        }
    }

    fn interval(&self) -> Option<SignedDuration> {
        self.state
            .current()
            .options
            .iter()
            .find_map(|command| command.interval())
            .map(SignedDuration::from_millis)
    }

    fn has(&self, command: Command) -> bool {
        if command.local() {
            self.state.current().has(command)
        } else {
            self.state.options.contains(&command)
        }
    }

    fn layout(&self) -> (Vec<Rect>, Rect, Rect) {
        let depth = self.depth();
        let panel = pages::panel(self.strip(), depth);
        (
            pages::rows(self.strip(), depth),
            panel,
            pages::content(panel),
        )
    }

    fn depth(&self) -> usize {
        let workspace = self.state.current();
        let path = workspace.path();
        let nested = (1..=path.len())
            .filter_map(|end| workspace.project_at(&path[..end]))
            .filter(|project| !project.projects.is_empty())
            .count();
        1 + nested
    }

    fn titles(&self, depth: usize) -> (Vec<String>, Option<usize>) {
        let workspace = self.state.current();
        if depth == 0 {
            let titles = workspace.pages.iter().map(|page| page.title().to_owned());
            return (titles.collect(), Some(workspace.page));
        }
        let path = workspace.path();
        let Some(project) = workspace.project_at(&path[..depth.min(path.len())]) else {
            return (Vec::new(), None);
        };
        let titles = project.projects.iter().map(|child| child.name.clone());
        (titles.collect(), project.page.checked_sub(1))
    }

    fn zoomed(&self) -> bool {
        self.state.window.is_some_and(|window| window.zoomed)
    }

    fn kind(&self) -> Kind {
        let workspace = self.state.current();
        match workspace.pages[workspace.page] {
            Page::Inbox => Kind::Inbox,
            Page::Performance => Kind::Performance,
            Page::Project(_) => Kind::Project,
        }
    }

    fn list(&self) -> Option<&List> {
        let workspace = self.state.current();
        match &workspace.pages[workspace.page] {
            Page::Inbox => Some(&workspace.list),
            Page::Project(_) => workspace.active().map(|project| &project.list),
            Page::Performance => None,
        }
    }

    fn list_mut(&mut self) -> Option<&mut List> {
        let workspace = self.state.current_mut();
        match workspace.pages[workspace.page] {
            Page::Inbox => Some(&mut workspace.list),
            Page::Project(_) => workspace.active_mut().map(|project| &mut project.list),
            Page::Performance => None,
        }
    }

    fn place(&self) -> Option<Place> {
        match self.kind() {
            Kind::Inbox => Some(Place::Inbox),
            Kind::Project => Some(Place::Project(self.state.current().path())),
            Kind::Performance => None,
        }
    }

    fn own(&self) -> u32 {
        system::own(self.state.current(), self.host.pid)
    }

    fn refresh(&mut self) {
        let now = self.now.timestamp();
        match self.kind() {
            Kind::Inbox => {
                let hidden = self.has(Command::HideSystemProcesses);
                let own = self.own();
                let workspace = self.state.current();
                let metrics = workspace.columns.clone();
                let levels: std::collections::HashMap<u32, usize> = workspace
                    .inbox
                    .iter()
                    .map(|item| (item.pid, item.level))
                    .collect();
                let rows = system::processes(workspace, &self.load, now, self.host.pid)
                    .iter()
                    .filter(|process| !hidden || !(process.pid <= SYSTEM || process.pid == own))
                    .map(|process| {
                        let (name, icon) = tag::split(process.name);
                        let process = Process {
                            name: &name,
                            ..*process
                        };
                        Row {
                            key: process.pid,
                            level: levels.get(&process.pid).copied().unwrap_or(0),
                            icon,
                            cells: metrics.iter().map(|metric| metric.cell(&process)).collect(),
                        }
                    })
                    .collect();
                let columns = metrics.iter().map(|metric| metric.column()).collect();
                self.state.current_mut().list.fill(columns, rows);
            }
            Kind::Project => {
                if let Some(project) = self.state.current_mut().active_mut() {
                    let rows = project
                        .tasks
                        .iter()
                        .map(|task| {
                            let (name, icon) = tag::split(&task.name);
                            Row {
                                key: task.id,
                                level: task.level,
                                icon,
                                cells: vec![name, task.status.text().to_owned()],
                            }
                        })
                        .collect();
                    project.list.fill(PROJECT.to_vec(), rows);
                }
            }
            Kind::Performance => {}
        }
    }

    fn readings(&self) -> Readings {
        let workspace = self.state.current();
        let totals = system::totals(workspace, self.now.timestamp());
        let committed = totals.commit;
        let limit = self.settings.memory;
        let samples = &workspace.history.samples;
        let series = |pick: fn(&crate::state::Sample) -> u8, live: u8| {
            samples.iter().map(pick).chain([live]).collect::<Vec<u8>>()
        };
        let percent = |memory: u64| {
            u8::try_from(memory * 100 / limit.max(1))
                .unwrap_or(100)
                .min(100)
        };
        let available = limit.saturating_sub(committed);
        Readings {
            cpu: series(|sample| sample.cpu, self.load.usage),
            kernel: self
                .has(Command::KernelTimes)
                .then(|| series(|sample| sample.kernel, self.load.kernel)),
            memory: samples
                .iter()
                .map(|sample| percent(sample.memory))
                .chain([percent(committed)])
                .collect(),
            usage: self.load.usage,
            committed,
            limit,
            midnight: system::midnight(workspace, self.now.time_zone()),
            blocks: [
                (
                    "Totals",
                    [
                        ("Handles", totals.handles),
                        ("Threads", totals.threads),
                        ("Processes", totals.processes),
                    ],
                ),
                (
                    "Physical Memory (K)",
                    [
                        ("Total", limit),
                        ("Available", available),
                        ("System Cache", available / 4),
                    ],
                ),
                (
                    "Commit Charge (K)",
                    [
                        ("Total", committed),
                        ("Limit", limit),
                        ("Peak", workspace.peak),
                    ],
                ),
                (
                    "Kernel Memory (K)",
                    [
                        ("Total", totals.paged + totals.nonpaged),
                        ("Paged", totals.paged),
                        ("Nonpaged", totals.nonpaged),
                    ],
                ),
            ],
        }
    }

    fn menus(&self) -> Vec<Menu> {
        let names: Vec<&str> = self
            .state
            .workspaces
            .iter()
            .map(|workspace| workspace.name.as_str())
            .collect();
        let nested = self.state.current().path().len() > 1;
        command::bar(self.kind(), nested, &names, |command| {
            self.settings
                .shortcuts
                .iter()
                .find(|&(_, &bound)| bound == command)
                .map(|(keystroke, _)| keystroke.to_string())
                .unwrap_or_default()
        })
    }

    fn hit(&self) -> Option<Target> {
        ui::hit(&self.targets, self.pointer)
    }

    fn sheet(&self) -> Option<&Sheet> {
        self.sheets.last()
    }

    fn hauling(&self) -> bool {
        matches!(self.pressed, Some(Target::Row(key))
            if self.moved && (self.spring.is_some() || self.draggable(key)))
    }

    fn ghost(&self) -> Option<(Rect, Canvas)> {
        let Some(Target::Row(key)) = self.pressed else {
            return None;
        };
        if !self.hauling() {
            return None;
        }
        let (canvas, above) = match &self.spring {
            Some(spring) => spring.sketch.clone(),
            None => self.sketch(key)?,
        };
        let size = canvas.size();
        let at = Rect::new(
            self.pointer.x - self.grab.x,
            self.pointer.y - self.grab.y - above,
            size.width,
            size.height,
        );
        Some((at, canvas))
    }

    fn sketch(&self, key: u32) -> Option<(Canvas, i32)> {
        let list = self.list()?;
        let grabbed = list.position(key)?;
        let rows: Vec<(usize, &Row)> = list
            .rows
            .iter()
            .enumerate()
            .filter(|&(index, row)| {
                list.selection.contains(&row.key) && index.abs_diff(grabbed) <= GHOST
            })
            .collect();
        let pitch = 14;
        let top = rows.iter().map(|&(index, _)| index).min()?;
        let bottom = rows.iter().map(|&(index, _)| index).max()?;
        let width = rows
            .iter()
            .map(|(_, row)| row.badge() + FONT.width(row.cells.first().map_or("", String::as_str)))
            .max()?;
        let height = i32::try_from(bottom - top + 1).ok()? * pitch;
        let mut canvas = Canvas::new(Size::new(width + 8, height), TRANSPARENT);
        for (index, row) in rows {
            let y = i32::try_from(index - top).ok()? * pitch;
            if let Some(icon) = row.icon {
                canvas.draw(icon, 3, y + 2);
            }
            let text = row.cells.first()?;
            FONT.draw(&mut canvas, text, 6 + row.badge(), y, BLACK);
        }
        canvas.fade();
        let above = i32::try_from(grabbed - top).ok()? * pitch;
        Some((canvas, above))
    }

    fn outline(&self) -> Option<(Rect, i32)> {
        if !self.moved {
            return None;
        }
        let by = Point::new(
            self.pointer.x - self.anchor.x,
            self.pointer.y - self.anchor.y,
        );
        match self.pressed? {
            Target::Caption if !self.zoomed() => Some((
                Rect::new(by.x, by.y, self.size.width, self.size.height),
                FRAME,
            )),
            Target::Frame => Some((
                chrome::stretch(
                    self.size,
                    chrome::edge(self.size, self.anchor),
                    by,
                    Self::MINIMUM,
                ),
                FRAME,
            )),
            Target::Dialog(Part::Caption) => {
                let sheet = self.sheet()?;
                let size = sheet.dialog.size();
                Some((
                    Rect::new(
                        sheet.at.x + by.x,
                        sheet.at.y + by.y,
                        size.width,
                        size.height,
                    ),
                    1,
                ))
            }
            _ => None,
        }
    }

    fn cursor(&self) -> Cursor {
        if self.hauling() {
            return if self.landing().is_some() {
                Cursor::Arrow
            } else {
                Cursor::No
            };
        }
        match self.pressed.or_else(|| self.hit()) {
            Some(Target::Frame) if self.menu.is_none() && self.sheets.is_empty() => {
                let at = if self.pressed.is_some() {
                    self.anchor
                } else {
                    self.pointer
                };
                Cursor::Resize(chrome::edge(self.size, at))
            }
            Some(Target::Divider(_)) if self.sheets.is_empty() => Cursor::Split,
            _ => Cursor::Arrow,
        }
    }

    fn blinking(&self) -> bool {
        self.active && self.sheet().is_some_and(|sheet| sheet.dialog.caret())
    }

    fn blinked(&self) -> i64 {
        let elapsed = self.now.timestamp().duration_since(self.blink).as_millis();
        i64::try_from(elapsed).unwrap_or_default().max(0)
    }

    fn caret(&self) -> bool {
        self.blinking() && (self.blinked() / BLINK) % 2 == 0
    }

    fn mark(&self, command: Command) -> menu::Mark {
        match command {
            Command::Workspace(ordinal) if ordinal == self.state.workspace + 1 => {
                menu::Mark::Bullet
            }
            _ if !self.has(command) => menu::Mark::Clear,
            Command::Speed(_) | Command::GraphPerProcessor => menu::Mark::Bullet,
            _ => menu::Mark::Check,
        }
    }

    fn enabled(&self, command: Command) -> bool {
        match command {
            Command::NewSubproject | Command::RenameProject | Command::DeleteProject => {
                self.kind() == Kind::Project
            }
            Command::DeleteWorkspace => self.state.workspaces.len() > 1,
            Command::EndTask | Command::SwitchTo | Command::EndProcess => {
                self.list().is_some_and(|list| !list.selection.is_empty())
            }
            _ => true,
        }
    }
}
