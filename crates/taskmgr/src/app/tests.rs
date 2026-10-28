use jiff::{SignedDuration, Zoned};
use tempfile::TempDir;

use super::*;
use crate::dialog::{Notice, Purpose, Reply};
use crate::metric::Metric;
use crate::scroll::Part as Bit;
use crate::state::Status;

struct Rig {
    app: App,
    now: Zoned,
    directory: TempDir,
}

const PLAIN: Modifiers = Modifiers {
    command: false,
    control: false,
    alt: false,
    shift: false,
};

const CMD: Modifiers = Modifiers {
    command: true,
    ..PLAIN
};
const CTRL: Modifiers = Modifiers {
    control: true,
    ..PLAIN
};
const ALT: Modifiers = Modifiers { alt: true, ..PLAIN };
const SHIFT: Modifiers = Modifiers {
    shift: true,
    ..PLAIN
};
const CTRL_SHIFT: Modifiers = Modifiers {
    shift: true,
    ..CTRL
};
const CMD_SHIFT: Modifiers = Modifiers { shift: true, ..CMD };

impl Rig {
    fn new() -> Self {
        let directory = TempDir::new().unwrap();
        let now: Zoned = "2026-10-27T09:30:00+01:00[Europe/Paris]".parse().unwrap();
        let app = App::new(Storage::new(directory.path()), host(), now.clone());
        let mut rig = Self {
            app,
            now,
            directory,
        };
        rig.app.scene();
        rig
    }

    fn reopen(self) -> Self {
        let app = App::new(
            Storage::new(self.directory.path()),
            host(),
            self.now.clone(),
        );
        let mut rig = Self { app, ..self };
        rig.app.scene();
        rig
    }

    fn send(&mut self, event: Event) -> Option<Effect> {
        let effect = self.app.handle(event, self.now.clone());
        self.app.scene();
        effect
    }

    fn later(&mut self, duration: SignedDuration) {
        self.now = self.now.checked_add(duration).unwrap();
        self.send(Event::Tick);
    }

    fn spot(&self, wanted: Target) -> Point {
        let &(rect, _) = self
            .app
            .targets
            .iter()
            .rev()
            .find(|&&(_, target)| target == wanted)
            .unwrap_or_else(|| panic!("{wanted:?} is not on screen"));
        Point::new(rect.x + rect.width / 2, rect.y + rect.height / 2)
    }

    fn press(&mut self, at: Point) -> Option<Effect> {
        self.send(Event::Move(at));
        self.send(Event::Press {
            at,
            clicks: 1,
            held: PLAIN,
        })
    }

    fn click(&mut self, at: Point) -> Option<Effect> {
        self.click_with(at, PLAIN)
    }

    fn click_with(&mut self, at: Point, held: Modifiers) -> Option<Effect> {
        self.send(Event::Move(at));
        let pressed = self.send(Event::Press {
            at,
            clicks: 1,
            held,
        });
        self.send(Event::Release(at)).or(pressed)
    }

    fn tap(&mut self, target: Target) -> Option<Effect> {
        self.click(self.spot(target))
    }

    fn double(&mut self, at: Point) -> Option<Effect> {
        self.click(at);
        let effect = self.send(Event::Press {
            at,
            clicks: 2,
            held: PLAIN,
        });
        self.send(Event::Release(at)).or(effect)
    }

    fn drag(&mut self, from: Point, to: Point) -> Option<Effect> {
        self.press(from);
        self.send(Event::Move(Point::new(from.x + 5, from.y)));
        self.send(Event::Move(to));
        self.send(Event::Release(to))
    }

    fn key(&mut self, key: Key, held: Modifiers) -> Option<Effect> {
        let down = self.send(Event::KeyDown(Keystroke::new(key, held)));
        self.send(Event::KeyUp(key)).or(down)
    }

    fn dialog(&self) -> Option<&Dialog> {
        self.app.sheets.last().map(|sheet| &sheet.dialog)
    }

    fn row(&self, index: usize) -> Point {
        let key = self.app.list().unwrap().rows[index].key;
        self.spot(Target::Row(key))
    }

    fn selected(&self) -> Vec<String> {
        let list = self.app.list().unwrap();
        list.rows
            .iter()
            .filter(|row| list.selection.contains(&row.key))
            .map(|row| row.cells[0].clone())
            .collect()
    }

    fn ask(&mut self) {
        self.app.run(Command::NewTask);
        self.app.scene();
    }

    fn enter(&mut self, text: &str) {
        self.key(Key::Enter, CMD);
        self.send(Event::Text(text.to_owned()));
        self.key(Key::Enter, PLAIN);
    }

    fn workspace(&self) -> &crate::state::Workspace {
        self.app.state.current()
    }

    fn titles(&self) -> Vec<String> {
        self.app.titles(0).0
    }

    fn cells(&self, column: usize) -> Vec<String> {
        self.app
            .list()
            .map(|list| {
                list.rows
                    .iter()
                    .map(|row| row.cells[column].clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn project(&mut self, name: &str) {
        self.key(Key::Character('n'), CMD);
        self.send(Event::Text(name.to_owned()));
        self.key(Key::Enter, PLAIN);
    }
}

fn host() -> Host {
    Host {
        user: "user".to_owned(),
        pid: 4242,
        seed: 1,
    }
}

#[test]
fn launches_with_an_inbox_of_three_processes_and_performance() {
    let rig = Rig::new();
    assert_eq!(rig.titles(), ["Inbox", "Performance"]);
    assert_eq!(
        rig.cells(0),
        ["System Idle Process", "System", "taskmgr.exe"]
    );
    assert_eq!(rig.cells(1), ["0", "8", "4242"]);
    assert_eq!(chrome::titles(&rig.app.menus()).count(), 4);
    assert_eq!(rig.app.title(), "Task Manager");
    assert!(!rig.app.floats() && rig.app.docked());
}

#[test]
fn new_task_asks_for_a_task_and_the_inbox_keeps_it() {
    let mut rig = Rig::new();
    rig.ask();
    assert!(matches!(rig.dialog(), Some(Dialog::Prompt(_))));
    rig.send(Event::Text("Call the bank".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert!(rig.dialog().is_none());
    assert_eq!(rig.cells(0)[2], "Call the bank");
    assert_eq!(rig.cells(1)[2], "9");
    assert_eq!(rig.cells(2)[2], "00 ");
    assert_eq!(rig.cells(4)[2], format!("{} K", 13 * 64));
    rig.later(SignedDuration::from_mins(61));
    assert_eq!(rig.cells(3)[2], "1:01:00");
    assert_eq!(rig.cells(4)[2], format!("{} K", 13 * 64 + 61));
    let idle = rig.cells(3)[0].clone();
    assert!(("1:00:00".."1:01:01").contains(&idle.as_str()), "{idle}");
    let rig = rig.reopen();
    assert_eq!(rig.workspace().inbox[0].name, "Call the bank");
    assert_eq!(rig.workspace().recent, ["Call the bank"]);
    assert_eq!(rig.cells(3)[0], "0:00:00");
}

#[test]
fn the_prompt_starts_from_the_last_entry_and_scrolls_through_history() {
    let mut rig = Rig::new();
    rig.enter("one");
    rig.enter("two");
    rig.ask();
    let Some(Dialog::Prompt(prompt)) = rig.dialog() else {
        panic!("the prompt should be open");
    };
    assert_eq!(prompt.field.selected(), Some("two"));
    rig.key(Key::Down, PLAIN);
    rig.key(Key::Escape, PLAIN);
    assert!(rig.dialog().is_none());
    rig.ask();
    rig.key(Key::Down, ALT);
    let choice = rig.spot(Target::Dialog(Part::Choice(1)));
    rig.click(choice);
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.workspace().inbox.len(), 3);
    assert_eq!(rig.workspace().inbox[2].name, "one");
}

#[test]
fn ending_the_system_is_refused_and_ending_taskmgr_quits() {
    let mut rig = Rig::new();
    rig.enter("Water the plants");
    assert_eq!(rig.key(Key::Backspace, PLAIN), None);
    assert_eq!(rig.workspace().inbox.len(), 0);
    rig.click(rig.spot(Target::Row(8)));
    assert_eq!(rig.key(Key::Delete, PLAIN), Some(Effect::Alert));
    let Some(Dialog::Message(message)) = rig.dialog() else {
        panic!("Windows should explain why");
    };
    assert_eq!(message.notice, Notice::Untouchable);
    assert_eq!(rig.dialog().unwrap().size(), Size::new(278, 126));
    rig.key(Key::Enter, PLAIN);
    assert!(rig.dialog().is_none());
    assert_eq!(rig.cells(0).len(), 3);
    rig.click(rig.spot(Target::Row(4242)));
    assert_eq!(
        rig.tap(Target::Button(Command::EndProcess)),
        Some(Effect::Close)
    );
}

#[test]
fn deleting_moves_the_selection_to_the_next_task() {
    let mut rig = Rig::new();
    for name in ["a", "b", "c"] {
        rig.enter(name);
    }
    rig.click(rig.spot(Target::Row(10)));
    rig.key(Key::Delete, PLAIN);
    assert_eq!(rig.selected(), ["c"]);
    rig.key(Key::Delete, PLAIN);
    assert_eq!(rig.selected(), ["a"]);
    rig.key(Key::Delete, PLAIN);
    assert_eq!(rig.selected().len(), 0);
    assert_eq!(rig.key(Key::Delete, PLAIN), Some(Effect::Beep));
}

#[test]
fn shift_and_command_clicks_select_several_rows() {
    let mut rig = Rig::new();
    rig.project("Home");
    for name in ["a", "b", "c", "d"] {
        rig.enter(name);
    }
    rig.click(rig.row(0));
    rig.click_with(rig.row(2), SHIFT);
    assert_eq!(rig.selected(), ["a", "b", "c"]);
    rig.click_with(rig.row(1), CMD);
    assert_eq!(rig.selected(), ["a", "c"]);
    rig.key(Key::Down, SHIFT);
    assert_eq!(rig.selected(), ["b", "c"]);
    rig.key(Key::Character('a'), CMD);
    assert_eq!(rig.selected().len(), 4);
    rig.tap(Target::Button(Command::EndTask));
    assert_eq!(rig.cells(0).len(), 0);
}

#[test]
fn arrows_walk_the_list_from_any_focus() {
    let mut rig = Rig::new();
    for name in ["a", "b"] {
        rig.enter(name);
    }
    rig.tap(Target::Tab(0, 0));
    rig.app.list_mut().unwrap().choose([]);
    rig.key(Key::Down, PLAIN);
    assert_eq!(rig.selected(), ["System Idle Process"]);
    rig.key(Key::End, PLAIN);
    assert_eq!(rig.selected(), ["taskmgr.exe"]);
    rig.key(Key::Up, SHIFT);
    assert_eq!(rig.selected(), ["b", "taskmgr.exe"]);
    rig.key(Key::PageUp, PLAIN);
    assert_eq!(rig.selected(), ["System Idle Process"]);
}

#[test]
fn projects_open_as_tabs_and_take_tasks_dragged_from_the_inbox() {
    let mut rig = Rig::new();
    rig.enter("Pack the bags");
    rig.project("Trip");
    assert_eq!(rig.titles(), ["Inbox", "Trip", "Performance"]);
    assert_eq!(rig.app.kind(), Kind::Project);
    rig.tap(Target::Tab(0, 0));
    let row = rig.spot(Target::Row(9));
    let tab = rig.spot(Target::Tab(0, 1));
    rig.press(row);
    rig.send(Event::Move(Point::new(row.x + 5, row.y - 40)));
    assert_eq!(rig.app.scene().cursor, Cursor::No);
    rig.send(Event::Move(tab));
    assert_eq!(
        rig.app.landing(),
        Some(Landing::Page(Place::Project(vec![1])))
    );
    rig.send(Event::Release(tab));
    assert_eq!(rig.workspace().inbox.len(), 0);
    let trip = rig.workspace().project(1).unwrap();
    assert_eq!(trip.tasks[0].name, "Pack the bags");
    assert_eq!(trip.tasks[0].status, Status::Stopped);
}

#[test]
fn tasks_change_status_and_end() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.enter("Fix the tap");
    rig.enter("Paint the door");
    assert_eq!(rig.cells(1), ["Stopped", "Stopped"]);
    rig.tap(Target::Button(Command::SwitchTo));
    assert_eq!(rig.cells(1), ["Stopped", "Running"]);
    let status = rig.row(0);
    let status = Point::new(rig.app.inside().unwrap().x + 260, status.y);
    rig.double(status);
    assert_eq!(rig.cells(1), ["Running", "Running"]);
    rig.double(status);
    assert_eq!(rig.cells(1), ["Not Responding", "Running"]);
    rig.send(Event::Press {
        at: status,
        clicks: 4,
        held: PLAIN,
    });
    rig.send(Event::Release(status));
    assert_eq!(rig.cells(1), ["Stopped", "Running"]);
    rig.tap(Target::Button(Command::EndTask));
    assert_eq!(rig.cells(0), ["Paint the door"]);
}

#[test]
fn dragging_shows_where_tasks_land_and_ends_sorting() {
    let mut rig = Rig::new();
    rig.project("Home");
    for task in ["b", "c", "a"] {
        rig.enter(task);
    }
    rig.tap(Target::Column(0));
    assert_eq!(rig.cells(0), ["a", "b", "c"]);
    let (first, last) = (rig.row(0), rig.row(2));
    rig.press(first);
    rig.send(Event::Move(Point::new(first.x, first.y + 5)));
    rig.send(Event::Move(Point::new(first.x, last.y + 5)));
    assert_eq!(
        rig.app.landing(),
        Some(Landing::Mark(Mark { index: 3, level: 0 }))
    );
    rig.send(Event::Release(Point::new(first.x, last.y + 5)));
    assert_eq!(rig.cells(0), ["b", "c", "a"]);
    assert_eq!(rig.app.list().unwrap().sorted, None);
    let names: Vec<&str> = rig
        .workspace()
        .project(1)
        .unwrap()
        .tasks
        .iter()
        .map(|task| task.name.as_str())
        .collect();
    assert_eq!(names, ["b", "c", "a"]);
}

#[test]
fn a_click_that_wobbles_does_not_reorder() {
    let mut rig = Rig::new();
    rig.project("Home");
    for task in ["b", "a"] {
        rig.enter(task);
    }
    rig.tap(Target::Column(0));
    let row = rig.row(0);
    rig.drag(row, Point::new(row.x + 1, row.y + 2));
    assert_eq!(rig.app.list().unwrap().sorted, Some((0, true)));
}

#[test]
fn tab_nests_tasks_one_level_at_a_time() {
    let mut rig = Rig::new();
    rig.project("Home");
    for task in ["Kitchen", "Fix the tap", "Buy a washer"] {
        rig.enter(task);
    }
    let levels = |rig: &Rig| -> Vec<usize> {
        rig.workspace()
            .project(1)
            .unwrap()
            .tasks
            .iter()
            .map(|task| task.level)
            .collect()
    };
    rig.click(rig.row(1));
    rig.key(Key::Tab, PLAIN);
    assert_eq!(rig.key(Key::Tab, PLAIN), Some(Effect::Beep));
    rig.click(rig.row(2));
    rig.key(Key::Tab, PLAIN);
    rig.key(Key::Tab, PLAIN);
    assert_eq!(levels(&rig), [0, 1, 2]);
    rig.click(rig.row(1));
    rig.key(Key::Tab, SHIFT);
    assert_eq!(levels(&rig), [0, 0, 1]);
    rig.click(rig.row(0));
    assert_eq!(
        rig.key(Key::Character('c'), CMD),
        Some(Effect::Copy("Kitchen".to_owned()))
    );
    rig.key(Key::Character('a'), CMD);
    assert_eq!(
        rig.key(Key::Character('c'), CMD),
        Some(Effect::Copy(
            "Kitchen\nFix the tap\n\tBuy a washer".to_owned()
        ))
    );
    rig.click(rig.row(1));
    rig.key(Key::Delete, PLAIN);
    assert_eq!(rig.cells(0), ["Kitchen"]);
}

#[test]
fn dragging_right_makes_a_subtask() {
    let mut rig = Rig::new();
    rig.project("Home");
    for task in ["Kitchen", "Fix the tap"] {
        rig.enter(task);
    }
    let row = rig.row(1);
    rig.press(row);
    rig.send(Event::Move(Point::new(row.x + 20, row.y + 1)));
    assert_eq!(
        rig.app.landing(),
        Some(Landing::Mark(Mark { index: 2, level: 1 }))
    );
    rig.send(Event::Release(Point::new(row.x + 20, row.y + 1)));
    let tasks = &rig.workspace().project(1).unwrap().tasks;
    assert_eq!(tasks[1].level, 1);
}

#[test]
fn tasks_cut_and_paste_between_projects() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.send(Event::Paste("Kitchen\n\tFix the tap\n\n".to_owned()));
    assert_eq!(rig.cells(0), ["Kitchen", "Fix the tap"]);
    assert_eq!(rig.selected().len(), 2);
    rig.click(rig.row(0));
    assert_eq!(
        rig.key(Key::Character('x'), CMD),
        Some(Effect::Copy("Kitchen\n\tFix the tap".to_owned()))
    );
    assert_eq!(rig.cells(0).len(), 0);
    rig.tap(Target::Tab(0, 0));
    rig.send(Event::Paste("Kitchen\n\tFix the tap".to_owned()));
    assert_eq!(rig.workspace().inbox.len(), 2);
    assert_eq!(rig.selected(), ["Kitchen", "Fix the tap"]);
}

#[test]
fn running_out_of_memory_refuses_new_tasks() {
    let mut rig = Rig::new();
    rig.app.settings.memory = system::commit(rig.workspace(), rig.now.timestamp()) + 64 * 4;
    rig.ask();
    rig.send(Event::Text("Too long a name".to_owned()));
    assert_eq!(rig.key(Key::Enter, PLAIN), Some(Effect::Alert));
    assert_eq!(rig.app.sheets.len(), 2);
    let Some(Dialog::Message(message)) = rig.dialog() else {
        panic!("Windows should complain about memory");
    };
    assert_eq!(message.notice, Notice::Memory);
    rig.key(Key::Enter, PLAIN);
    assert!(matches!(rig.dialog(), Some(Dialog::Prompt(_))));
    rig.key(Key::Character('a'), CMD);
    rig.send(Event::Text("Tiny".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert!(rig.dialog().is_none());
    assert_eq!(rig.workspace().inbox[0].name, "Tiny");
    assert_eq!(
        rig.send(Event::Paste("More".to_owned())),
        Some(Effect::Alert)
    );
}

#[test]
fn projects_are_renamed_and_deleted_from_the_file_menu() {
    let mut rig = Rig::new();
    assert!(!rig.app.enabled(Command::DeleteProject));
    rig.project("Hmoe");
    rig.app.run(Command::RenameProject);
    rig.key(Key::Character('a'), CMD);
    rig.send(Event::Text("Home".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.titles(), ["Inbox", "Home", "Performance"]);
    assert_eq!(rig.app.run(Command::DeleteProject), Some(Effect::Alert));
    assert!(matches!(rig.dialog(), Some(Dialog::Message(_))));
    rig.key(Key::Right, PLAIN);
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.titles(), ["Inbox", "Home", "Performance"]);
    rig.app.run(Command::DeleteProject);
    rig.app.scene();
    rig.tap(Target::Dialog(Part::Reply(Reply::Yes)));
    assert_eq!(rig.titles(), ["Inbox", "Performance"]);
}

#[test]
fn enter_on_a_focused_button_pushes_it_until_released() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.enter("Fix the tap");
    assert_eq!(rig.app.focus, Focus::List);
    let button = rig.spot(Target::Button(Command::EndTask));
    rig.press(button);
    rig.send(Event::Move(Point::new(button.x, button.y - 60)));
    rig.send(Event::Release(Point::new(button.x, button.y - 60)));
    assert_eq!(rig.app.focus, Focus::Button(Command::EndTask));
    assert_eq!(rig.cells(0).len(), 1);
    rig.send(Event::KeyDown(Keystroke::new(Key::Enter, PLAIN)));
    assert_eq!(
        rig.app.keyed,
        Some((Key::Enter, Target::Button(Command::EndTask)))
    );
    assert_eq!(rig.cells(0).len(), 1);
    rig.send(Event::KeyUp(Key::Enter));
    assert_eq!(rig.cells(0).len(), 0);
}

#[test]
fn select_columns_adds_columns_to_every_inbox() {
    let mut rig = Rig::new();
    rig.app.run(Command::SelectColumns);
    rig.app.scene();
    assert_eq!(rig.dialog().unwrap().size(), Size::new(299, 347));
    rig.tap(Target::Dialog(Part::Check(16)));
    rig.tap(Target::Dialog(Part::Check(6)));
    rig.tap(Target::Dialog(Part::Check(2)));
    rig.tap(Target::Dialog(Part::Reply(Reply::Ok)));
    assert_eq!(
        rig.workspace().columns,
        [
            Metric::Name,
            Metric::Pid,
            Metric::CpuTime,
            Metric::Memory,
            Metric::PeakMemory
        ]
    );
    let titles: Vec<&str> = rig
        .app
        .list()
        .unwrap()
        .columns
        .iter()
        .map(|column| column.title)
        .collect();
    assert_eq!(
        titles,
        [
            "Image Name",
            "PID",
            "CPU Time",
            "Mem Usage",
            "Peak Mem Usage"
        ]
    );
    assert_eq!(rig.cells(4), ["16 K", "1,256 K", "2,104 K"]);
}

#[test]
fn options_hide_the_system_the_dock_and_the_window() {
    let mut rig = Rig::new();
    rig.app.run(Command::HideSystemProcesses);
    rig.app.scene();
    assert_eq!(rig.cells(0).len(), 0);
    assert_eq!(
        rig.app.run(Command::HideDockIcon),
        Some(Effect::Dock(false))
    );
    assert_eq!(rig.tap(Target::Minimize), Some(Effect::Minimize));
    rig.app.run(Command::HideWhenMinimized);
    assert_eq!(rig.tap(Target::Minimize), Some(Effect::Hide));
    let rig = rig.reopen();
    assert!(!rig.app.docked());
}

#[test]
fn tabs_reorder_by_dragging_and_stay_that_way() {
    let mut rig = Rig::new();
    rig.project("Home");
    let (home, inbox) = (rig.spot(Target::Tab(0, 1)), rig.spot(Target::Tab(0, 0)));
    rig.drag(home, Point::new(inbox.x - 10, inbox.y));
    assert_eq!(rig.titles(), ["Home", "Inbox", "Performance"]);
    let rig = rig.reopen();
    assert_eq!(rig.titles(), ["Home", "Inbox", "Performance"]);
    assert_eq!(rig.workspace().page, 0);
}

#[test]
fn workspaces_keep_their_own_inboxes_and_switch_by_number() {
    let mut rig = Rig::new();
    rig.enter("Home chores");
    rig.app.run(Command::NewWorkspace);
    rig.send(Event::Text("Work".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.app.state.workspace, 1);
    assert_eq!(rig.app.title(), "Task Manager (Work)");
    assert_eq!(rig.workspace().inbox.len(), 0);
    rig.enter("Write the report");
    assert_eq!(rig.workspace().inbox[0].pid, 9);
    assert_eq!(rig.key(Key::Character('1'), CMD), None);
    assert_eq!(rig.workspace().inbox[0].name, "Home chores");
    assert_eq!(rig.key(Key::Character('3'), CMD), Some(Effect::Beep));
    rig.app.run(Command::RenameWorkspace);
    rig.send(Event::Text("Home".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.app.title(), "Task Manager (Home)");
    let view = &rig.app.menus()[2].entries;
    let Some(menu::Entry::Submenu(_, list)) = view.last() else {
        panic!("View should end with Workspaces");
    };
    let labels: Vec<&str> = list.iter().map(menu::Entry::label).collect();
    assert_eq!(
        labels,
        [
            "&New Workspace...",
            "&Rename Workspace...",
            "&Delete Workspace",
            "",
            "&1 Home",
            "&2 Work"
        ]
    );
    assert_eq!(rig.app.mark(Command::Workspace(1)), menu::Mark::Bullet);
}

#[test]
fn the_outline_appears_once_the_caption_moves_and_the_window_follows() {
    let mut rig = Rig::new();
    let caption = Point::new(200, 10);
    rig.press(caption);
    assert!(rig.app.outline().is_none());
    rig.send(Event::Move(Point::new(201, 10)));
    assert!(rig.app.outline().is_some());
    rig.send(Event::Move(Point::new(190, 4)));
    let scene = rig.app.scene();
    let (area, outline) = scene.popups.last().unwrap();
    assert_eq!(*area, Rect::new(-10, -6, 404, 447));
    assert_eq!(outline.pixel(200, 200), [0, 0, 0, 0]);
    assert_eq!(outline.pixel(0, 0), [0, 0, 0, 255]);
    assert_eq!(outline.pixel(1, 0), [255, 255, 255, 255]);
    let [red, green, blue, alpha] = scene.window.pixel(392, 94);
    assert_eq!(outline.pixel(402, 100), [!red, !green, !blue, alpha]);
    let place = rig.send(Event::Release(Point::new(190, 4)));
    assert_eq!(place, Some(Effect::Place(*area)));
}

#[test]
fn the_frame_resizes_no_smaller_than_the_minimum_and_escape_cancels() {
    let mut rig = Rig::new();
    rig.press(Point::new(403, 446));
    assert_eq!(rig.app.scene().cursor, Cursor::Resize(Edge { x: 1, y: 1 }));
    rig.send(Event::Move(Point::new(423, 476)));
    let place = rig.send(Event::Release(Point::new(423, 476)));
    assert_eq!(place, Some(Effect::Place(Rect::new(0, 0, 424, 477))));
    rig.press(Point::new(1, 200));
    rig.send(Event::Move(Point::new(300, 200)));
    let place = rig.send(Event::Release(Point::new(300, 200)));
    assert_eq!(place, Some(Effect::Place(Rect::new(84, 0, 320, 447))));
    rig.press(Point::new(200, 10));
    rig.send(Event::Move(Point::new(260, 90)));
    rig.send(Event::KeyDown(Keystroke::new(Key::Escape, PLAIN)));
    assert_eq!(rig.send(Event::Release(Point::new(260, 90))), None);
}

#[test]
fn zooming_is_remembered_and_stops_dragging() {
    let mut rig = Rig::new();
    rig.send(Event::Placed(Rect::new(100, 100, 404, 447)));
    let at = Point::new(200, 10);
    assert_eq!(
        rig.send(Event::Press {
            at,
            clicks: 2,
            held: PLAIN
        }),
        Some(Effect::Zoom(true))
    );
    rig.send(Event::Release(at));
    rig.press(at);
    rig.send(Event::Move(Point::new(260, 90)));
    assert!(rig.app.outline().is_none());
    let rig = rig.reopen();
    assert_eq!(
        rig.app.placement(),
        Some(Placement {
            frame: Rect::new(100, 100, 404, 447),
            zoomed: true
        })
    );
}

#[test]
fn dialogs_are_modal_and_about_closes_with_ok() {
    let mut rig = Rig::new();
    rig.app.run(Command::About);
    rig.app.scene();
    assert_eq!(rig.click(Point::new(200, 20)), Some(Effect::Beep));
    assert_eq!(rig.click(Point::new(200, 200)), None);
    let scene = rig.app.scene();
    assert_eq!(scene.popups.len(), 1);
    assert_eq!(scene.popups[0].0.size(), Size::new(419, 251));
    rig.tap(Target::Dialog(Part::Reply(Reply::Ok)));
    assert!(rig.dialog().is_none());
}

#[test]
fn the_prompt_edits_like_a_mac_text_field() {
    let mut rig = Rig::new();
    rig.ask();
    rig.send(Event::Text("Ship".to_owned()));
    rig.send(Event::Text(" it".to_owned()));
    rig.send(Event::Paste(" now".to_owned()));
    rig.key(Key::Backspace, ALT);
    rig.key(Key::Left, SHIFT);
    let copied = rig.key(Key::Character('c'), CMD);
    assert_eq!(copied, Some(Effect::Copy(" ".to_owned())));
    rig.key(Key::Character('z'), CMD);
    rig.key(Key::Character('z'), CMD);
    let Some(Dialog::Prompt(prompt)) = rig.dialog() else {
        panic!("the prompt should be open");
    };
    assert_eq!(prompt.field.text(), "Ship it");
    rig.key(Key::Character('z'), CMD);
    rig.key(Key::Character('z'), CMD_SHIFT);
    rig.key(Key::Character('a'), CMD);
    assert_eq!(
        rig.key(Key::Character('x'), CMD),
        Some(Effect::Copy("Ship it".to_owned()))
    );
    rig.send(Event::Text("Plan".to_owned()));
    let field = rig.spot(Target::Dialog(Part::Field));
    rig.send(Event::Press {
        at: field,
        clicks: 3,
        held: PLAIN,
    });
    rig.send(Event::Release(field));
    rig.send(Event::Text("Done".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.workspace().inbox[0].name, "Done");
}

#[test]
fn ok_waits_for_a_name() {
    let mut rig = Rig::new();
    rig.app.run(Command::NewProject);
    rig.app.scene();
    rig.key(Key::Enter, PLAIN);
    assert!(rig.dialog().is_some());
    rig.tap(Target::Dialog(Part::Close));
    assert!(rig.dialog().is_none());
    assert_eq!(rig.titles(), ["Inbox", "Performance"]);
}

#[test]
fn menus_drop_where_windows_drops_them() {
    let mut rig = Rig::new();
    rig.click(Point::new(15, 31));
    let scene = rig.app.scene();
    assert_eq!(scene.popups.len(), 1);
    assert_eq!((scene.popups[0].0.x, scene.popups[0].0.y), (4, 41));
    rig.click(Point::new(15, 31));
    assert_eq!(rig.app.scene().popups.len(), 0);
}

#[test]
fn always_on_top_is_off_until_chosen() {
    let mut rig = Rig::new();
    assert!(!rig.app.floats());
    rig.click(Point::new(50, 31));
    assert_eq!(rig.click(Point::new(60, 52)), Some(Effect::Float(true)));
    assert!(rig.app.floats());
    assert!(rig.reopen().app.floats());
}

#[test]
fn submenus_wait_before_opening() {
    let mut rig = Rig::new();
    rig.tap(Target::Tab(0, 1));
    rig.click(Point::new(95, 31));
    let start = rig.now.clone();
    rig.send(Event::Move(Point::new(120, 70)));
    let due = rig.app.menu.as_ref().and_then(|menu| menu.due).unwrap();
    assert_eq!(
        due.duration_since(start.timestamp()),
        SignedDuration::from_millis(400)
    );
    rig.later(SignedDuration::from_millis(100));
    assert_eq!(rig.app.scene().popups.len(), 1);
    rig.later(SignedDuration::from_millis(300));
    assert_eq!(rig.app.scene().popups.len(), 2);
}

#[test]
fn alt_walks_the_menu_bar_with_the_keyboard() {
    let mut rig = Rig::new();
    rig.key(Key::Alt, ALT);
    let menu = rig.app.menu.as_ref().unwrap();
    assert!(menu.keyboard && !menu.dropped());
    rig.key(Key::Down, PLAIN);
    assert_eq!(rig.app.scene().popups.len(), 1);
    rig.key(Key::Right, PLAIN);
    assert_eq!(rig.app.scene().popups.len(), 2);
    assert_eq!(
        rig.app.menu.as_ref().unwrap().help,
        Command::NewProject.help()
    );
    rig.key(Key::Escape, PLAIN);
    assert_eq!(rig.key(Key::Character('x'), PLAIN), Some(Effect::Close));
}

#[test]
fn control_tab_cycles_pages_and_shows_focus() {
    let mut rig = Rig::new();
    rig.key(Key::Tab, CTRL);
    assert_eq!((rig.workspace().page, rig.app.cues.focus), (1, true));
    rig.key(Key::Tab, CTRL_SHIFT);
    assert_eq!(rig.workspace().page, 0);
}

#[test]
fn caption_buttons_act_on_release_inside() {
    let mut rig = Rig::new();
    assert_eq!(rig.click(Point::new(390, 12)), Some(Effect::Close));
    rig.press(Point::new(355, 12));
    rig.send(Event::Move(Point::new(200, 200)));
    assert_eq!(rig.send(Event::Release(Point::new(200, 200))), None);
}

#[test]
fn headers_sort_the_inbox_and_keep_the_selection() {
    let mut rig = Rig::new();
    rig.enter("a long name for a task");
    rig.enter("b");
    rig.tap(Target::Column(4));
    assert_eq!(
        rig.cells(0),
        [
            "System Idle Process",
            "b",
            "System",
            "a long name for a task",
            "taskmgr.exe"
        ]
    );
    assert_eq!(rig.app.list().unwrap().index(), Some(1));
    rig.tap(Target::Column(4));
    assert_eq!(rig.cells(0)[0], "taskmgr.exe");
    rig.tap(Target::Column(0));
    assert_eq!(rig.cells(0)[0], "a long name for a task");
    rig.tap(Target::Column(1));
    assert_eq!(rig.cells(1), ["0", "8", "9", "10", "4242"]);
}

#[test]
fn small_windows_scroll_their_lists() {
    let mut rig = Rig::new();
    for index in 0..20 {
        rig.enter(&format!("task {index}"));
    }
    rig.send(Event::Resize(App::MINIMUM));
    assert!(
        rig.app
            .targets
            .iter()
            .any(|&(_, target)| target == Target::Scroll(true, Bit::Thumb))
    );
    assert!(rig.app.list().unwrap().scrolled[1] > 0);
    rig.key(Key::Home, PLAIN);
    let at = Point::new(150, 150);
    rig.send(Event::Scroll {
        at,
        by: Point::new(0, 20),
    });
    rig.send(Event::Scroll {
        at,
        by: Point::new(30, 22),
    });
    assert_eq!(rig.app.list().unwrap().scrolled, [30, 3]);
}

#[test]
fn the_performance_page_draws_a_day_per_grid_square() {
    let mut rig = Rig::new();
    rig.enter("Plan the week");
    rig.later(SignedDuration::from_hours(24 * 5));
    let workspace = rig.workspace();
    assert_eq!(workspace.history.samples.len(), 30);
    assert!(workspace.history.samples.last().unwrap().cpu > 1);
    assert!(workspace.peak > 12_000);
    rig.tap(Target::Tab(0, 1));
    let readings = rig.app.readings();
    assert_eq!(readings.cpu.len(), 31);
    assert_eq!(readings.midnight, 2);
    let [totals, physical, commit, kernel] = readings.blocks;
    assert_eq!(totals.1[2], ("Processes", 4));
    assert_eq!(physical.1[0], ("Total", 523_828));
    assert_eq!(commit.1[1], ("Limit", 523_828));
    assert_eq!(kernel.1[0].1, kernel.1[1].1 + kernel.1[2].1);
}

#[test]
fn workspaces_are_deleted_after_a_warning() {
    let mut rig = Rig::new();
    assert!(!rig.app.enabled(Command::DeleteWorkspace));
    rig.app.run(Command::NewWorkspace);
    rig.send(Event::Text("Work".to_owned()));
    rig.key(Key::Enter, PLAIN);
    rig.enter("Write the report");
    assert_eq!(rig.app.run(Command::DeleteWorkspace), Some(Effect::Alert));
    rig.key(Key::Escape, PLAIN);
    assert_eq!(rig.app.state.workspaces.len(), 2);
    rig.app.run(Command::DeleteWorkspace);
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.app.state.workspaces.len(), 1);
    assert_eq!(rig.app.title(), "Task Manager");
    assert_eq!(rig.workspace().inbox.len(), 0);
    assert_eq!(rig.app.run(Command::DeleteWorkspace), Some(Effect::Beep));
}

#[test]
fn taskmgr_cannot_be_started_twice() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.key(Key::Enter, PLAIN);
    rig.send(Event::Text(" TASKMGR.EXE ".to_owned()));
    assert_eq!(rig.key(Key::Enter, PLAIN), Some(Effect::Alert));
    let Some(Dialog::Message(message)) = rig.dialog() else {
        panic!("taskmgr needs a licence");
    };
    assert_eq!(message.notice, Notice::License);
    rig.key(Key::Enter, PLAIN);
    rig.key(Key::Escape, PLAIN);
    assert_eq!(
        rig.send(Event::Paste("Shop\ntaskmgr @red".to_owned())),
        Some(Effect::Alert)
    );
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.cells(0).len(), 0);
}

#[test]
fn enter_adds_the_next_task_beside_the_selected_one() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.send(Event::Paste(
        "Kitchen\n\tFix the tap\n\t\tBuy a washer\nGarden".to_owned(),
    ));
    rig.click(rig.row(1));
    rig.enter("Paint the cupboards");
    rig.click(rig.row(0));
    rig.enter("Hall");
    let tasks: Vec<(usize, &str)> = rig
        .workspace()
        .project(1)
        .unwrap()
        .tasks
        .iter()
        .map(|task| (task.level, task.name.as_str()))
        .collect();
    assert_eq!(
        tasks,
        [
            (0, "Kitchen"),
            (1, "Fix the tap"),
            (2, "Buy a washer"),
            (1, "Paint the cupboards"),
            (0, "Hall"),
            (0, "Garden"),
        ]
    );
    assert_eq!(rig.selected(), ["Hall"]);
}

#[test]
fn command_z_takes_edits_back_and_shows_where() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.enter("Fix the tap");
    rig.enter("Paint the door");
    rig.key(Key::Delete, PLAIN);
    rig.tap(Target::Tab(0, 0));
    assert_eq!(rig.key(Key::Character('z'), CMD), None);
    assert_eq!(rig.workspace().page, 1);
    assert_eq!(rig.cells(0), ["Fix the tap", "Paint the door"]);
    assert_eq!(rig.selected(), ["Paint the door"]);
    rig.key(Key::Character('z'), CMD_SHIFT);
    assert_eq!(rig.cells(0), ["Fix the tap"]);
    for _ in 0..4 {
        rig.key(Key::Character('z'), CMD);
    }
    assert_eq!(rig.titles(), ["Inbox", "Performance"]);
    assert_eq!(rig.key(Key::Character('z'), CMD), Some(Effect::Beep));
}

#[test]
fn double_clicking_a_project_tab_renames_it() {
    let mut rig = Rig::new();
    rig.project("Hmoe");
    let tab = rig.spot(Target::Tab(0, 1));
    rig.double(tab);
    assert!(matches!(rig.dialog(), Some(Dialog::Prompt(_))));
    rig.send(Event::Text("Home".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.titles(), ["Inbox", "Home", "Performance"]);
    rig.double(rig.spot(Target::Tab(0, 0)));
    assert!(rig.dialog().is_none());
}

#[test]
fn long_lists_follow_the_selection_and_scroll_while_dragging() {
    let mut rig = Rig::new();
    rig.send(Event::Resize(App::MINIMUM));
    rig.project("Home");
    for index in 0..20 {
        rig.enter(&format!("task {index}"));
    }
    let scrolled = rig.app.list().unwrap().scrolled[1];
    assert!(scrolled > 0);
    rig.key(Key::Up, CMD);
    assert_eq!(rig.app.list().unwrap().scrolled[1], 0);
    let inside = rig.app.inside().unwrap();
    let bottom = rig.app.list().unwrap().layout(inside).body.bottom() - 2;
    let first = rig.row(0);
    rig.press(first);
    rig.send(Event::Move(Point::new(first.x, first.y + 10)));
    rig.send(Event::Move(Point::new(first.x, bottom)));
    assert!(
        rig.app
            .wake()
            .is_some_and(|wake| wake < rig.now.timestamp() + SignedDuration::from_millis(100))
    );
    for _ in 0..5 {
        rig.later(SignedDuration::from_millis(50));
    }
    assert_eq!(rig.app.list().unwrap().scrolled[1], 5);
    rig.send(Event::Release(Point::new(first.x, bottom)));
    assert_ne!(rig.cells(0)[0], "task 0");
}

#[test]
fn column_dividers_drag_a_line_and_resize_on_release() {
    let mut rig = Rig::new();
    let divider = rig.spot(Target::Divider(0));
    let width = rig.app.list().unwrap().widths[0];
    rig.press(divider);
    rig.send(Event::Move(Point::new(divider.x + 20, divider.y + 100)));
    assert_eq!(rig.app.list().unwrap().widths[0], width);
    let inside = rig.app.inside().unwrap();
    let scene = rig.app.scene();
    assert_eq!(
        scene.window.pixel(divider.x + 20, inside.y + 200),
        [0, 0, 0, 255]
    );
    rig.send(Event::Release(Point::new(divider.x + 20, divider.y + 100)));
    assert_eq!(rig.app.list().unwrap().widths[0], width + 20);
}

#[test]
fn inbox_items_nest_and_move_to_projects_with_their_subtasks() {
    let mut rig = Rig::new();
    rig.project("Trips");
    rig.tap(Target::Tab(0, 0));
    rig.send(Event::Paste(
        "Rome\n\tBook the flight\n\tPack\nMilk".to_owned(),
    ));
    let levels: Vec<usize> = rig
        .workspace()
        .inbox
        .iter()
        .map(|item| item.level)
        .collect();
    assert_eq!(levels, [0, 1, 1, 0]);
    rig.click(rig.row(5));
    rig.key(Key::Tab, PLAIN);
    assert_eq!(rig.workspace().inbox[3].level, 1);
    rig.key(Key::Tab, SHIFT);
    let rome = rig.row(2);
    let tab = rig.spot(Target::Tab(0, 1));
    rig.press(rome);
    rig.send(Event::Move(Point::new(rome.x + 5, rome.y + 40)));
    rig.send(Event::Move(tab));
    rig.send(Event::Release(tab));
    let names: Vec<&str> = rig
        .workspace()
        .inbox
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(names, ["Milk"]);
    let tasks: Vec<(usize, &str)> = rig
        .workspace()
        .project(1)
        .unwrap()
        .tasks
        .iter()
        .map(|task| (task.level, task.name.as_str()))
        .collect();
    assert_eq!(tasks, [(0, "Rome"), (1, "Book the flight"), (1, "Pack")]);
}

#[test]
fn each_workspace_keeps_its_own_columns_and_speed() {
    let mut rig = Rig::new();
    rig.app.run(Command::Speed(crate::command::Speed::High));
    rig.app.run(Command::SelectColumns);
    rig.app.scene();
    rig.tap(Target::Dialog(Part::Check(6)));
    rig.tap(Target::Dialog(Part::Reply(Reply::Ok)));
    rig.app.run(Command::AlwaysOnTop);
    rig.app.run(Command::NewWorkspace);
    rig.send(Event::Text("Work".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.workspace().columns, crate::metric::DEFAULT);
    assert_eq!(rig.app.interval(), Some(SignedDuration::from_secs(2)));
    assert!(rig.app.floats());
    rig.key(Key::Character('1'), CMD);
    assert_eq!(rig.workspace().columns.len(), 6);
    assert_eq!(rig.app.interval(), Some(SignedDuration::from_millis(500)));
}

#[test]
fn space_switches_status_and_tab_walks_the_tabs() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.enter("Fix the tap");
    rig.key(Key::Space, PLAIN);
    assert_eq!(rig.cells(1), ["Running"]);
    rig.key(Key::Space, PLAIN);
    assert_eq!(rig.cells(1), ["Not Responding"]);
    rig.tap(Target::Tab(0, 1));
    rig.key(Key::Tab, PLAIN);
    assert_eq!(rig.titles()[rig.workspace().page], "Performance");
    rig.key(Key::Tab, SHIFT);
    rig.key(Key::Tab, SHIFT);
    assert_eq!(rig.workspace().page, 0);
    rig.key(Key::Down, PLAIN);
    assert_eq!(rig.app.focus, Focus::List);
    assert_eq!(rig.selected(), ["System Idle Process"]);
    rig.key(Key::Up, PLAIN);
    assert_eq!(rig.app.focus, Focus::Tabs(0));
}

#[test]
fn subprojects_line_up_as_a_row_of_tabs_under_their_project() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.enter("Call the plumber");
    rig.key(Key::Character('n'), CMD_SHIFT);
    rig.send(Event::Text("Kitchen".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.app.titles(1), (vec!["Kitchen".to_owned()], None));
    assert_eq!(rig.workspace().path(), [1]);
    rig.key(Key::Character('n'), CMD_SHIFT);
    rig.send(Event::Text("Bathroom".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.app.titles(1).0, ["Kitchen", "Bathroom"]);
    assert_eq!(rig.cells(0), ["Call the plumber"]);
    rig.tap(Target::Tab(1, 0));
    assert_eq!(rig.workspace().path(), [1, 0]);
    rig.enter("Fix the tap");
    let file = &rig.app.menus()[0].entries;
    let labels: Vec<&str> = file.iter().map(menu::Entry::label).collect();
    assert_eq!(
        labels,
        [
            "&New",
            "&Rename Subproject...",
            "&Delete Subproject",
            "",
            "E&xit Task Manager"
        ]
    );
    rig.tap(Target::Tab(0, 1));
    assert_eq!(rig.workspace().path(), [1]);
    assert_eq!(rig.app.titles(1).1, None);
    assert_eq!(rig.cells(0), ["Call the plumber"]);
    rig.tap(Target::Tab(1, 0));
    assert_eq!(rig.cells(0), ["Fix the tap"]);
    rig.key(Key::Character('n'), CMD_SHIFT);
    rig.send(Event::Text("Sink".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.app.depth(), 3);
    assert_eq!(rig.workspace().path(), [1, 0]);
    rig.tap(Target::Tab(1, 0));
    assert_eq!(rig.app.depth(), 3);
    assert_eq!(rig.app.titles(2), (vec!["Sink".to_owned()], None));
    let row = rig.row(0);
    let home = rig.spot(Target::Tab(0, 1));
    rig.press(row);
    rig.send(Event::Move(Point::new(row.x, row.y + 40)));
    rig.send(Event::Move(home));
    assert_eq!(
        rig.app.landing(),
        Some(Landing::Page(Place::Project(vec![1])))
    );
    rig.send(Event::Release(home));
    let names: Vec<&str> = rig
        .workspace()
        .project(1)
        .unwrap()
        .tasks
        .iter()
        .map(|task| task.name.as_str())
        .collect();
    assert_eq!(names, ["Call the plumber", "Fix the tap"]);
    rig.double(rig.spot(Target::Tab(1, 0)));
    rig.key(Key::Character('a'), CMD);
    rig.send(Event::Text("Cooking".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.app.titles(1).0, ["Cooking", "Bathroom"]);
    rig.tap(Target::Tab(2, 0));
    assert_eq!(rig.app.run(Command::DeleteProject), Some(Effect::Alert));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.app.depth(), 2);
    assert_eq!(rig.workspace().path(), [1, 0]);
    rig.key(Key::Character('z'), CMD);
    assert_eq!(rig.app.depth(), 3);
}

#[test]
fn swiping_over_the_tabs_scrolls_them() {
    let mut rig = Rig::new();
    for index in 0..8 {
        rig.project(&format!("Project {index}"));
    }
    let tab = rig.spot(Target::Tab(0, 8));
    assert!(rig.app.first[0] > 2);
    rig.send(Event::Scroll {
        at: tab,
        by: Point::new(-500, 0),
    });
    assert_eq!(rig.app.first[0], 0);
    for _ in 0..3 {
        rig.send(Event::Scroll {
            at: tab,
            by: Point::new(20, 0),
        });
    }
    assert_eq!(rig.app.first[0], 2);
    rig.send(Event::Scroll {
        at: tab,
        by: Point::new(-30, 0),
    });
    assert_eq!(rig.app.first[0], 1);
}

#[test]
fn typing_jumps_to_the_task_that_starts_with_it() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.send(Event::Paste(
        "Fix the tap\nFeed the cat\nПочинить кран".to_owned(),
    ));
    rig.send(Event::Text("f".to_owned()));
    assert_eq!(rig.selected(), ["Fix the tap"]);
    rig.send(Event::Text("e".to_owned()));
    assert_eq!(rig.selected(), ["Feed the cat"]);
    rig.later(SignedDuration::from_secs(-60));
    rig.send(Event::Text("п".to_owned()));
    assert_eq!(rig.selected(), ["Починить кран"]);
    rig.key(Key::F2, PLAIN);
    let Some(Dialog::Prompt(prompt)) = rig.dialog() else {
        panic!("F2 should rename");
    };
    assert_eq!(prompt.field.text(), "Починить кран");
}

#[test]
fn deleting_everything_in_the_inbox_spares_the_system_and_taskmgr() {
    let mut rig = Rig::new();
    rig.enter("a");
    rig.enter("b");
    rig.key(Key::Character('a'), CMD);
    assert_eq!(rig.selected(), ["a", "b"]);
    rig.click_with(rig.spot(Target::Row(4242)), CMD);
    assert_eq!(rig.key(Key::Delete, PLAIN), None);
    assert_eq!(rig.workspace().inbox.len(), 0);
    rig.tap(Target::Tab(0, 0));
    assert_eq!(rig.key(Key::Delete, PLAIN), Some(Effect::Beep));
}

#[test]
fn keys_wait_while_rows_are_dragged() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.enter("a");
    rig.enter("b");
    let row = rig.row(0);
    rig.press(row);
    rig.send(Event::Move(Point::new(row.x, row.y + 30)));
    assert_eq!(rig.key(Key::Character('1'), CMD), None);
    assert_eq!(rig.workspace().page, 1);
    rig.send(Event::Paste("c".to_owned()));
    assert_eq!(rig.cells(0).len(), 2);
    rig.key(Key::Escape, PLAIN);
    rig.send(Event::Release(Point::new(row.x, row.y + 30)));
    assert_eq!(rig.cells(0), ["a", "b"]);
}

#[test]
fn deep_subprojects_still_draw_in_the_smallest_window() {
    let mut rig = Rig::new();
    rig.send(Event::Resize(App::MINIMUM));
    rig.project("Home");
    for depth in 0..8 {
        rig.key(Key::Character('n'), CMD_SHIFT);
        rig.send(Event::Text(format!("Level {depth}")));
        rig.key(Key::Enter, PLAIN);
        rig.tap(Target::Tab(depth + 1, 0));
        rig.enter("task");
    }
    assert_eq!(rig.app.depth(), 9);
    rig.app.scene();
    rig.key(Key::Up, PLAIN);
    rig.key(Key::Up, PLAIN);
    assert!(matches!(rig.app.focus, Focus::Tabs(_)));
}

#[test]
fn an_open_menu_keeps_letters_and_pastes_away_from_the_list() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.enter("alpha");
    rig.enter("beta");
    rig.key(Key::F10, PLAIN);
    rig.send(Event::Text("a".to_owned()));
    rig.send(Event::Paste("gamma".to_owned()));
    rig.key(Key::Escape, PLAIN);
    assert_eq!(rig.cells(0), ["alpha", "beta"]);
    assert_eq!(rig.selected(), ["beta"]);
}

#[test]
fn turning_the_clock_back_keeps_the_graphs_going() {
    let mut rig = Rig::new();
    rig.later(SignedDuration::from_secs(5));
    rig.later(SignedDuration::from_hours(-1));
    assert_eq!(rig.app.sampled, Some(rig.now.timestamp()));
}

#[test]
fn holding_a_dragged_task_over_a_tab_opens_it_down_to_its_subprojects() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.key(Key::Character('n'), CMD_SHIFT);
    rig.send(Event::Text("Repairs".to_owned()));
    rig.key(Key::Enter, PLAIN);
    rig.tap(Target::Tab(1, 0));
    rig.enter("a");
    rig.enter("b");
    rig.tap(Target::Tab(0, 0));
    rig.enter("Fix the tap");
    rig.enter("Feed the cat");
    let pid = rig.workspace().inbox[0].pid;
    let from = rig.spot(Target::Row(pid));
    rig.press(from);
    rig.send(Event::Move(Point::new(from.x + 5, from.y)));
    rig.send(Event::Move(rig.spot(Target::Tab(0, 1))));
    assert_eq!(rig.app.place(), Some(Place::Inbox));
    rig.later(SignedDuration::from_millis(600));
    assert_eq!(rig.app.place(), Some(Place::Project(vec![1])));
    assert_eq!(rig.app.titles(1).0, ["Repairs"]);
    rig.send(Event::Move(rig.spot(Target::Tab(1, 0))));
    rig.later(SignedDuration::from_millis(600));
    assert_eq!(rig.app.place(), Some(Place::Project(vec![1, 0])));
    let between = rig.row(1);
    let between = Point::new(between.x, between.y - 3);
    rig.send(Event::Move(between));
    assert!(matches!(rig.app.landing(), Some(Landing::Into(_))));
    rig.send(Event::Release(between));
    assert_eq!(rig.app.place(), Some(Place::Inbox));
    assert_eq!(rig.selected(), ["Feed the cat"]);
    let repairs = rig.workspace().project_at(&[1, 0]).unwrap();
    let names: Vec<&str> = repairs
        .tasks
        .iter()
        .map(|task| task.name.as_str())
        .collect();
    assert_eq!(names, ["a", "Fix the tap", "b"]);
    rig.key(Key::Character('z'), CMD);
    assert_eq!(rig.workspace().inbox.len(), 2);
    assert_eq!(rig.workspace().project_at(&[1, 0]).unwrap().tasks.len(), 2);
}

#[test]
fn letting_go_or_escaping_after_a_tab_opened_comes_back_unchanged() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.tap(Target::Tab(0, 0));
    rig.enter("Fix the tap");
    let from = rig.spot(Target::Row(rig.workspace().inbox[0].pid));
    for escape in [false, true] {
        rig.press(from);
        rig.send(Event::Move(Point::new(from.x + 5, from.y)));
        rig.send(Event::Move(rig.spot(Target::Tab(0, 1))));
        rig.later(SignedDuration::from_millis(600));
        assert_eq!(rig.app.place(), Some(Place::Project(vec![1])));
        let outside = Point::new(2, rig.app.size.height - 2);
        rig.send(Event::Move(outside));
        if escape {
            rig.key(Key::Escape, PLAIN);
        }
        rig.send(Event::Release(outside));
        assert_eq!(rig.app.place(), Some(Place::Inbox));
        assert_eq!(rig.workspace().inbox.len(), 1);
    }
    rig.press(from);
    rig.send(Event::Move(Point::new(from.x + 5, from.y)));
    let tab = rig.spot(Target::Tab(0, 1));
    rig.send(Event::Move(tab));
    rig.later(SignedDuration::from_millis(600));
    rig.send(Event::Release(tab));
    assert_eq!(rig.app.place(), Some(Place::Inbox));
    assert_eq!(rig.workspace().inbox.len(), 0);
    assert_eq!(rig.workspace().project_at(&[1]).unwrap().tasks.len(), 1);
}

#[test]
fn color_tags_put_a_ball_before_the_name_in_the_inbox_and_in_projects() {
    let mut rig = Rig::new();
    rig.enter("Renew passport @red");
    rig.enter("Buy milk");
    let icons = |rig: &Rig| -> Vec<bool> {
        let list = rig.app.list().unwrap();
        list.rows.iter().map(|row| row.icon.is_some()).collect()
    };
    assert_eq!(icons(&rig), [false, false, true, false, false]);
    rig.project("Home");
    rig.enter("Fix the tap");
    assert_eq!(icons(&rig), [false]);
    rig.key(Key::F2, PLAIN);
    rig.key(Key::End, PLAIN);
    rig.send(Event::Text(" @blue".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(icons(&rig), [true]);
    assert_eq!(rig.cells(0), ["Fix the tap"]);
    assert_eq!(
        rig.key(Key::Character('c'), CMD),
        Some(Effect::Copy("Fix the tap @blue".to_owned()))
    );
    let at = rig.row(0);
    rig.press(at);
    rig.send(Event::Move(Point::new(at.x + 5, at.y + 40)));
    let scene = rig.app.scene();
    let (_, ghost) = scene.popups.last().unwrap();
    assert_eq!(ghost.size().width, FONT.width("Fix the tap") + 14 + 8);
}

#[test]
fn return_edits_the_selected_task_and_command_return_adds_one_after_it() {
    let mut rig = Rig::new();
    rig.project("Home");
    rig.enter("Fix the tap @blue");
    rig.enter("Feed the cat");
    rig.click(rig.row(0));
    rig.key(Key::Enter, PLAIN);
    let Some(Dialog::Prompt(prompt)) = rig.dialog() else {
        panic!("Return should edit the selected task");
    };
    assert_eq!(prompt.purpose, Purpose::Rename(0));
    assert_eq!(prompt.field.selected(), Some("Fix the tap @blue"));
    rig.key(Key::End, PLAIN);
    rig.send(Event::Text(" now".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(rig.cells(0), ["Fix the tap now", "Feed the cat"]);
    rig.key(Key::Enter, CMD);
    let Some(Dialog::Prompt(prompt)) = rig.dialog() else {
        panic!("Command-Return should ask for a new task");
    };
    assert_eq!(prompt.purpose, Purpose::Task);
    rig.key(Key::Character('a'), CMD);
    rig.send(Event::Text("Call the plumber".to_owned()));
    rig.key(Key::Enter, PLAIN);
    assert_eq!(
        rig.cells(0),
        ["Fix the tap now", "Call the plumber", "Feed the cat"]
    );
    assert_eq!(rig.selected(), ["Call the plumber"]);
    rig.click(Point::new(rig.row(2).x, rig.row(2).y + 60));
    rig.key(Key::Enter, PLAIN);
    assert!(
        matches!(rig.dialog(), Some(Dialog::Prompt(prompt)) if prompt.purpose == Purpose::Task)
    );
}

#[test]
fn return_on_a_system_process_beeps_and_performance_still_asks() {
    let mut rig = Rig::new();
    rig.click(rig.spot(Target::Row(8)));
    assert_eq!(rig.key(Key::Enter, PLAIN), Some(Effect::Beep));
    assert!(rig.dialog().is_none());
    rig.tap(Target::Tab(0, 1));
    rig.key(Key::Enter, PLAIN);
    assert!(matches!(rig.dialog(), Some(Dialog::Prompt(_))));
}

#[test]
fn option_dropping_a_tab_on_another_makes_it_a_subproject() {
    let mut rig = Rig::new();
    rig.project("Work");
    rig.project("Clippy");
    rig.enter("Animate the paperclip");
    assert_eq!(rig.titles(), ["Inbox", "Work", "Clippy", "Performance"]);
    let clippy = rig.spot(Target::Tab(0, 2));
    let work = rig.spot(Target::Tab(0, 1));
    rig.press(clippy);
    rig.send(Event::KeyDown(Keystroke::new(Key::Alt, ALT)));
    rig.send(Event::Move(Point::new(clippy.x - 5, clippy.y)));
    rig.send(Event::Move(work));
    assert_eq!(rig.titles(), ["Inbox", "Work", "Clippy", "Performance"]);
    assert!(matches!(rig.app.regroup(), Some(Regroup::Into { .. })));
    rig.send(Event::Release(work));
    rig.send(Event::KeyUp(Key::Alt));
    assert!(rig.app.menu.is_none());
    assert_eq!(rig.titles(), ["Inbox", "Work", "Performance"]);
    assert_eq!(rig.workspace().path(), [1, 0]);
    assert_eq!(rig.app.titles(1), (vec!["Clippy".to_owned()], Some(0)));
    assert_eq!(rig.cells(0), ["Animate the paperclip"]);
    rig.key(Key::Character('z'), CMD);
    assert_eq!(rig.titles(), ["Inbox", "Work", "Clippy", "Performance"]);
}

#[test]
fn dragging_a_subproject_tab_up_a_row_makes_it_a_project_again() {
    let mut rig = Rig::new();
    rig.project("Work");
    rig.key(Key::Character('n'), CMD_SHIFT);
    rig.send(Event::Text("Clippy".to_owned()));
    rig.key(Key::Enter, PLAIN);
    rig.key(Key::Character('n'), CMD_SHIFT);
    rig.send(Event::Text("Sink".to_owned()));
    rig.key(Key::Enter, PLAIN);
    rig.tap(Target::Tab(1, 0));
    rig.key(Key::Character('n'), CMD_SHIFT);
    rig.send(Event::Text("Balloons".to_owned()));
    rig.key(Key::Enter, PLAIN);
    let clippy = rig.spot(Target::Tab(1, 0));
    rig.press(clippy);
    rig.send(Event::Move(Point::new(clippy.x, clippy.y + 5)));
    let below = rig.spot(Target::Tab(2, 0));
    rig.send(Event::Move(below));
    assert_eq!(rig.app.regroup(), None);
    let performance = rig.spot(Target::Tab(0, 2));
    let between = Point::new(performance.x - 20, performance.y);
    rig.send(Event::Move(between));
    assert!(matches!(
        rig.app.regroup(),
        Some(Regroup::Among {
            level: 0,
            gap: 2,
            ..
        })
    ));
    let tabs: Vec<Target> = rig
        .app
        .targets
        .iter()
        .map(|&(_, target)| target)
        .filter(|target| matches!(target, Target::Tab(1.., _)))
        .collect();
    assert_eq!(tabs, [Target::Tab(1, 1)]);
    rig.app.cues.focus = false;
    let plain = rig.app.scene().window;
    rig.app.cues.focus = true;
    assert_eq!(rig.app.scene().window.rgba(), plain.rgba());
    rig.send(Event::Release(between));
    assert_eq!(rig.titles(), ["Inbox", "Work", "Clippy", "Performance"]);
    assert_eq!(rig.workspace().path(), [2]);
    assert_eq!(rig.app.titles(1).0, ["Balloons"]);
    rig.tap(Target::Tab(0, 1));
    assert_eq!(rig.app.titles(1).0, ["Sink"]);
}
