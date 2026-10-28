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

mod draw;

use crate::assets::{ERROR, FONT, WARNING};
use crate::field::{Field, Motion};
use crate::geometry::{Point, Rect, Size};
use crate::graphics::Image;
use crate::keys::{Key, Keystroke, Modifiers};
use crate::metric::{CHOICES, Metric};
use crate::ui::Cues;

pub(crate) use draw::{draw, draw_dropdown};

const LINE: i32 = 13;
const BUTTON: Size = Size::new(75, 23);
const PROMPT: Size = Size::new(347, 179);
const ABOUT: Size = Size::new(419, 251);
const COLUMNS: Size = Size::new(299, 347);
const COMBO: Rect = Rect::new(57, 82, 275, 21);
const SHOWN: usize = 8;
const TEXT: i32 = 65;
const ICON: i32 = 32;
const TOP: i32 = 33;
const WRAP: i32 = 268;
const RIGHT: i32 = 11;
const SPACING: i32 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Body,
    Caption,
    Help,
    Close,
    Field,
    Arrow,
    Choice(usize),
    Reply(Reply),
    Check(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reply {
    Ok,
    Cancel,
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Purpose {
    Task,
    Rename(u32),
    Project,
    Subproject,
    RenameProject(bool),
    Workspace,
    RenameWorkspace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Notice {
    DeleteProject(bool),
    DeleteWorkspace(usize),
    Untouchable,
    Memory,
    License,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Control {
    Field,
    Reply(Reply),
    Check(usize),
}

#[derive(Debug)]
pub(crate) enum Outcome {
    Stay,
    Reply(Reply),
    Copy(String),
}

#[derive(Debug)]
pub(crate) struct Prompt {
    pub(crate) purpose: Purpose,
    pub(crate) field: Field,
    pub(crate) recent: Vec<String>,
    pub(crate) dropped: bool,
    pub(crate) highlight: Option<usize>,
    pub(crate) focus: Control,
}

#[derive(Debug)]
pub(crate) struct About {
    pub(crate) licensee: String,
    pub(crate) memory: u64,
}

#[derive(Debug)]
pub(crate) struct Message {
    pub(crate) notice: Notice,
    lines: Vec<String>,
    width: i32,
    focus: Reply,
}

#[derive(Debug)]
pub(crate) struct Columns {
    pub(crate) checked: [bool; CHOICES.len()],
    focus: Control,
}

#[derive(Debug)]
pub(crate) enum Dialog {
    Prompt(Prompt),
    About(About),
    Message(Message),
    Columns(Columns),
}

#[derive(Clone, Copy, Debug)]
struct Button {
    reply: Reply,
    label: &'static str,
    area: Rect,
    enabled: bool,
}

impl Purpose {
    fn title(self) -> &'static str {
        match self {
            Self::Task => "Create New Task",
            Self::Rename(_) => "Edit Task",
            Self::Project => "Create New Project",
            Self::Subproject => "Create New Subproject",
            Self::RenameProject(false) => "Rename Project",
            Self::RenameProject(true) => "Rename Subproject",
            Self::Workspace => "Create New Workspace",
            Self::RenameWorkspace => "Rename Workspace",
        }
    }

    fn message(self) -> [&'static str; 2] {
        match self {
            Self::Task => [
                "Type the name of a program, folder, document, or",
                "Internet resource, and Windows will open it for you.",
            ],
            Self::Rename(_) => [
                "Type the new name of the task, and Windows will",
                "rename it for you.",
            ],
            Self::Project => [
                "Type the name of a project, and Windows will create",
                "it for you.",
            ],
            Self::Subproject => [
                "Type the name of a subproject, and Windows will create",
                "it inside the current project for you.",
            ],
            Self::RenameProject(_) => [
                "Type the new name of the project, and Windows will",
                "rename it for you.",
            ],
            Self::Workspace => [
                "Type the name of a workspace, and Windows will create",
                "a new inbox and projects for you.",
            ],
            Self::RenameWorkspace => [
                "Type the new name of the workspace, and Windows will",
                "rename it for you.",
            ],
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Task | Self::Rename(_) => "&Open:",
            _ => "&Name:",
        }
    }
}

impl Notice {
    fn title(self) -> &'static str {
        match self {
            Self::DeleteProject(_) | Self::DeleteWorkspace(_) => "Task Manager Warning",
            Self::Untouchable => "Unable to Terminate Process",
            Self::Memory => "Unable to Create Task",
            Self::License => "Task Manager",
        }
    }

    fn icon(self) -> &'static Image {
        match self {
            Self::DeleteProject(_) | Self::DeleteWorkspace(_) => &WARNING,
            Self::Untouchable | Self::Memory | Self::License => &ERROR,
        }
    }

    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::DeleteProject(false) => {
                "WARNING: Deleting a project can cause undesired results including loss of data. \
                 The tasks and subprojects in the project will be deleted along with it.  \
                 Are you sure you want to delete the project?"
            }
            Self::DeleteProject(true) => {
                "WARNING: Deleting a subproject can cause undesired results including loss of \
                 data. The tasks and subprojects in it will be deleted along with it.  \
                 Are you sure you want to delete the subproject?"
            }
            Self::DeleteWorkspace(_) => {
                "WARNING: Deleting a workspace can cause undesired results including loss of data. \
                 Its inbox, projects and history will be deleted along with it and cannot be \
                 restored.  Are you sure you want to delete the workspace?"
            }
            Self::Untouchable => {
                "The operation could not be completed.\n\nThe operation is not valid for this process."
            }
            Self::License => "Oi mate! Ya got a loicense fer dat?",
            Self::Memory => {
                "The operation could not be completed.\n\nNot enough storage is available to complete this operation."
            }
        }
    }

    const fn replies(self) -> &'static [(Reply, &'static str)] {
        match self {
            Self::DeleteProject(_) | Self::DeleteWorkspace(_) => {
                &[(Reply::Yes, "Yes"), (Reply::No, "No")]
            }
            Self::Untouchable | Self::Memory | Self::License => &[(Reply::Ok, "OK")],
        }
    }
}

impl Prompt {
    pub(crate) fn new(purpose: Purpose, text: String, recent: Vec<String>) -> Self {
        Self {
            purpose,
            field: Field::new(text),
            recent,
            dropped: false,
            highlight: None,
            focus: Control::Field,
        }
    }

    pub(crate) fn text(&self) -> &str {
        self.field.text().trim()
    }

    fn drop(&mut self) {
        self.dropped = true;
        self.highlight = self.recent.iter().position(|entry| entry == self.text());
    }

    fn choose(&mut self, index: usize) {
        if let Some(text) = self.recent.get(index) {
            self.field.replace(text.clone());
        }
    }

    fn pick(&mut self, key: Key, alt: bool) {
        let closing = matches!(key, Key::Enter | Key::Escape | Key::F4 | Key::Tab);
        if closing || (alt && matches!(key, Key::Up | Key::Down)) {
            self.dropped = false;
            return;
        }
        let next = match (key, self.highlight) {
            (Key::Up, Some(index)) => index.saturating_sub(1),
            (Key::Down, Some(index)) => index + 1,
            (Key::Up | Key::Down, None) => 0,
            _ => return,
        };
        if next < self.recent.len() {
            self.choose(next);
            self.highlight = Some(next);
        }
    }

    fn edit(&mut self, key: Key, held: Modifiers) -> Outcome {
        let field = &mut self.field;
        let Modifiers {
            command,
            control,
            alt,
            shift,
        } = held;
        match key {
            Key::Character('z') if command && shift => {
                field.redo();
            }
            Key::Character('z') if command => {
                field.undo();
            }
            Key::Character('a') if command => field.select_all(),
            Key::Character('c') if command => {
                return field
                    .selected()
                    .map_or(Outcome::Stay, |text| Outcome::Copy(text.to_owned()));
            }
            Key::Character('x') if command => {
                return field.cut().map_or(Outcome::Stay, Outcome::Copy);
            }
            Key::Character('a') if control => field.go(Motion::Start, shift),
            Key::Character('e') if control => field.go(Motion::End, shift),
            Key::Character('b') if control => field.go(Motion::Back, shift),
            Key::Character('f') if control => field.go(Motion::Forth, shift),
            Key::Character('h') if control => field.erase(Motion::Back),
            Key::Character('d') if control => field.erase(Motion::Forth),
            Key::Character('k') if control => {
                field.go(Motion::End, true);
                return field.cut().map_or(Outcome::Stay, Outcome::Copy);
            }
            Key::F4 => self.drop(),
            Key::Up | Key::Down if alt => self.drop(),
            Key::Up | Key::Down if command || shift => {
                let motion = if key == Key::Up {
                    Motion::Start
                } else {
                    Motion::End
                };
                field.go(motion, shift);
            }
            Key::Up | Key::Down => {
                let current = self.recent.iter().position(|entry| entry == self.text());
                let next = match (key, current) {
                    (Key::Up, Some(index)) => index.checked_sub(1),
                    (Key::Down, Some(index)) => Some(index + 1),
                    (_, None) => Some(0),
                    _ => None,
                };
                if let Some(index) = next.filter(|&index| index < self.recent.len()) {
                    self.choose(index);
                }
            }
            Key::Left | Key::Right | Key::Home | Key::End => {
                let back = matches!(key, Key::Left | Key::Home);
                let motion = match (key, command, alt) {
                    (Key::Home | Key::End, ..) | (_, true, _) if back => Motion::Start,
                    (Key::Home | Key::End, ..) | (_, true, _) => Motion::End,
                    (_, _, true) if back => Motion::WordBack,
                    (_, _, true) => Motion::WordForth,
                    _ if back => Motion::Back,
                    _ => Motion::Forth,
                };
                field.go(motion, shift);
            }
            Key::Backspace | Key::Delete => {
                let back = key == Key::Backspace;
                let motion = match (command, alt) {
                    (true, _) if back => Motion::Start,
                    (true, _) => Motion::End,
                    (_, true) if back => Motion::WordBack,
                    (_, true) => Motion::WordForth,
                    _ if back => Motion::Back,
                    _ => Motion::Forth,
                };
                field.erase(motion);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl Message {
    pub(crate) fn new(notice: Notice) -> Self {
        let (lines, width) = layout(notice.text(), notice.replies().len());
        Self {
            notice,
            lines,
            width,
            focus: notice.replies()[0].0,
        }
    }

    fn height(&self) -> i32 {
        LINE * i32::try_from(self.lines.len()).unwrap_or_default()
    }

    fn text(&self) -> i32 {
        TOP + (ICON - self.height()).max(0) / 2
    }

    fn top(&self) -> i32 {
        TOP + self.height().max(ICON) + 17
    }
}

impl Columns {
    pub(crate) fn new(shown: &[Metric]) -> Self {
        Self {
            checked: CHOICES
                .map(|(_, metric)| metric.is_some_and(|metric| shown.contains(&metric))),
            focus: Control::Check(1),
        }
    }

    pub(crate) fn chosen(&self) -> Vec<Metric> {
        CHOICES
            .iter()
            .zip(self.checked)
            .filter(|&(_, checked)| checked)
            .filter_map(|(&(_, metric), _)| metric)
            .collect()
    }

    pub(crate) fn enabled(index: usize) -> bool {
        index > 0
            && CHOICES
                .get(index)
                .is_some_and(|(_, metric)| metric.is_some())
    }

    fn toggle(&mut self, index: usize) {
        if Self::enabled(index) {
            self.checked[index] = !self.checked[index];
        }
    }

    fn step(&mut self, forth: bool) -> bool {
        let Control::Check(index) = self.focus else {
            return false;
        };
        let next = if forth {
            (index + 1..CHOICES.len()).find(|&index| Self::enabled(index))
        } else {
            (1..index).rev().find(|&index| Self::enabled(index))
        };
        next.map(|index| self.focus = Control::Check(index))
            .is_some()
    }
}

impl Dialog {
    pub(crate) fn size(&self) -> Size {
        match self {
            Self::Prompt(_) => PROMPT,
            Self::About(_) => ABOUT,
            Self::Columns(_) => COLUMNS,
            Self::Message(message) => Size::new(message.width, message.top() + BUTTON.height + 14),
        }
    }

    pub(crate) fn title(&self) -> &'static str {
        match self {
            Self::Prompt(prompt) => prompt.purpose.title(),
            Self::About(_) => "About Task Manager",
            Self::Message(message) => message.notice.title(),
            Self::Columns(_) => "Select Columns",
        }
    }

    pub(crate) fn dropdown(&self, at: Point) -> Option<Rect> {
        let Self::Prompt(prompt) = self else {
            return None;
        };
        if !prompt.dropped {
            return None;
        }
        let rows = i32::try_from(prompt.recent.len().clamp(1, SHOWN)).unwrap_or(1);
        Some(Rect::new(
            at.x + COMBO.x,
            at.y + COMBO.bottom(),
            COMBO.width,
            rows * LINE + 2,
        ))
    }

    pub(crate) fn caret(&self) -> bool {
        matches!(self, Self::Prompt(prompt) if prompt.focus == Control::Field && !prompt.dropped)
    }

    fn buttons(&self) -> Vec<Button> {
        let at = |reply, label, x, y| Button {
            reply,
            label,
            area: Rect::new(x, y, BUTTON.width, BUTTON.height),
            enabled: true,
        };
        match self {
            Self::Prompt(prompt) => vec![
                Button {
                    enabled: !prompt.text().is_empty(),
                    ..at(Reply::Ok, "OK", 177, 136)
                },
                at(Reply::Cancel, "Cancel", 258, 136),
            ],
            Self::About(_) => vec![at(Reply::Ok, "OK", 333, 217)],
            Self::Columns(_) => vec![
                at(Reply::Ok, "OK", 128, 308),
                at(Reply::Cancel, "Cancel", 209, 308),
            ],
            Self::Message(message) => {
                let replies = message.notice.replies();
                let count = i32::try_from(replies.len()).unwrap_or(1);
                let row = count * BUTTON.width + (count - 1) * SPACING;
                let left = (message.width - row) / 2;
                (0..)
                    .zip(replies)
                    .map(|(index, &(reply, label))| {
                        let x = left + index * (BUTTON.width + SPACING);
                        at(reply, label, x, message.top())
                    })
                    .collect()
            }
        }
    }

    fn focus(&self) -> Control {
        match self {
            Self::Prompt(prompt) => prompt.focus,
            Self::About(_) => Control::Reply(Reply::Ok),
            Self::Message(message) => Control::Reply(message.focus),
            Self::Columns(columns) => columns.focus,
        }
    }

    fn set_focus(&mut self, control: Control) {
        match (self, control) {
            (Self::Prompt(prompt), control) => {
                prompt.focus = control;
                if control == Control::Field {
                    prompt.field.select_all();
                }
            }
            (Self::Message(message), Control::Reply(reply)) => message.focus = reply,
            (Self::Columns(columns), control) => columns.focus = control,
            _ => {}
        }
    }

    fn default(&self) -> Option<Reply> {
        match self.focus() {
            Control::Reply(reply) => Some(reply),
            Control::Field | Control::Check(_) => Some(Reply::Ok),
        }
        .filter(|&reply| self.enabled(reply))
    }

    fn dismiss(&self) -> Reply {
        match self {
            Self::Prompt(_) | Self::Columns(_) => Reply::Cancel,
            Self::About(_) => Reply::Ok,
            Self::Message(message) => message
                .notice
                .replies()
                .last()
                .map_or(Reply::Ok, |&(reply, _)| reply),
        }
    }

    pub(crate) fn closable(&self) -> bool {
        !matches!(self, Self::Message(message) if message.notice.replies().len() > 1)
    }

    fn enabled(&self, reply: Reply) -> bool {
        self.buttons()
            .iter()
            .any(|button| button.reply == reply && button.enabled)
    }

    fn reply(&self, reply: Reply) -> Outcome {
        if self.enabled(reply) {
            Outcome::Reply(reply)
        } else {
            Outcome::Stay
        }
    }

    fn stops(&self) -> Vec<Control> {
        let mut stops = match self {
            Self::Prompt(_) => vec![Control::Field],
            Self::Columns(_) => (0..CHOICES.len())
                .filter(|&index| Columns::enabled(index))
                .map(Control::Check)
                .collect(),
            Self::About(_) | Self::Message(_) => Vec::new(),
        };
        stops.extend(
            self.buttons()
                .iter()
                .filter(|button| button.enabled)
                .map(|button| Control::Reply(button.reply)),
        );
        stops
    }

    fn cycle(&mut self, step: isize) {
        let stops = self.stops();
        let Some(count) = isize::try_from(stops.len()).ok().filter(|&count| count > 0) else {
            return;
        };
        let index = stops
            .iter()
            .position(|&stop| stop == self.focus())
            .and_then(|index| isize::try_from(index).ok())
            .unwrap_or(0);
        let next = usize::try_from((index + step).rem_euclid(count)).unwrap_or(0);
        self.set_focus(stops[next]);
    }

    pub(crate) fn pressable(&self, key: Key) -> Option<Reply> {
        match self.focus() {
            Control::Reply(reply) if matches!(key, Key::Enter | Key::Space) => {
                Some(reply).filter(|&reply| self.enabled(reply))
            }
            _ => None,
        }
    }

    pub(crate) fn press(&mut self, part: Part, x: i32, clicks: u8) {
        if let Self::Prompt(prompt) = self
            && prompt.dropped
            && !matches!(part, Part::Choice(_) | Part::Arrow)
        {
            prompt.dropped = false;
            return;
        }
        match (self, part) {
            (Self::Prompt(prompt), Part::Field) => {
                prompt.focus = Control::Field;
                let index = prompt.field.index_at(x - COMBO.x - 6 + prompt.field.shift);
                match clicks {
                    2 => prompt.field.select_word(index),
                    3.. => prompt.field.select_all(),
                    _ => prompt.field.place(index, false),
                }
            }
            (Self::Prompt(prompt), Part::Arrow) => {
                prompt.focus = Control::Field;
                if prompt.dropped {
                    prompt.dropped = false;
                } else {
                    prompt.drop();
                }
            }
            (Self::Columns(columns), Part::Check(index)) if Columns::enabled(index) => {
                columns.focus = Control::Check(index);
            }
            (dialog, Part::Reply(reply)) if dialog.enabled(reply) => {
                dialog.set_focus(Control::Reply(reply));
            }
            _ => {}
        }
    }

    pub(crate) fn drag(&mut self, part: Option<Part>, x: i32, held: bool) {
        let Self::Prompt(prompt) = self else {
            return;
        };
        match part {
            Some(Part::Choice(index)) => prompt.highlight = Some(index),
            _ if held && prompt.focus == Control::Field && !prompt.dropped => {
                let index = prompt.field.index_at(x - COMBO.x - 6 + prompt.field.shift);
                prompt.field.place(index, true);
            }
            _ => {}
        }
    }

    pub(crate) fn release(&mut self, pressed: Option<Part>, part: Option<Part>) -> Outcome {
        match (self, part) {
            (Self::Prompt(prompt), Some(Part::Choice(index))) => {
                prompt.choose(index);
                prompt.dropped = false;
                Outcome::Stay
            }
            (Self::Columns(columns), Some(Part::Check(index))) if pressed == part => {
                columns.toggle(index);
                Outcome::Stay
            }
            (dialog, Some(Part::Reply(reply))) if pressed == part => dialog.reply(reply),
            (dialog, Some(Part::Close)) if pressed == part && dialog.closable() => {
                Outcome::Reply(dialog.dismiss())
            }
            _ => Outcome::Stay,
        }
    }

    pub(crate) fn key(&mut self, keystroke: Keystroke, cues: &mut Cues) -> Outcome {
        let Keystroke { key, held } = keystroke;
        if let Self::Prompt(prompt) = self
            && prompt.dropped
        {
            prompt.pick(key, held.alt);
            return Outcome::Stay;
        }
        match key {
            Key::Escape => return Outcome::Reply(self.dismiss()),
            Key::Enter => return self.default().map_or(Outcome::Stay, Outcome::Reply),
            Key::Tab => {
                self.cycle(if held.shift { -1 } else { 1 });
                cues.focus = true;
                return Outcome::Stay;
            }
            _ => {}
        }
        match self {
            Self::Prompt(prompt) => match key {
                Key::Character('o' | 'n') if held.alt => {
                    prompt.focus = Control::Field;
                    prompt.field.select_all();
                }
                Key::Space if prompt.focus != Control::Field => {}
                _ if prompt.focus == Control::Field => return prompt.edit(key, held),
                _ => {}
            },
            Self::Columns(columns) => match (key, columns.focus) {
                (Key::Space, Control::Check(index)) => columns.toggle(index),
                (Key::Up | Key::Left | Key::Down | Key::Right, _) => {
                    cues.focus |= columns.step(matches!(key, Key::Down | Key::Right));
                }
                _ => {}
            },
            Self::Message(_) if matches!(key, Key::Left | Key::Right | Key::Up | Key::Down) => {
                let forth = matches!(key, Key::Right | Key::Down);
                self.cycle(if forth { 1 } else { -1 });
                cues.focus = true;
            }
            Self::About(_) | Self::Message(_) => {}
        }
        Outcome::Stay
    }

    pub(crate) fn type_in(&mut self, text: &str, paste: bool) {
        if let Self::Prompt(prompt) = self
            && prompt.focus == Control::Field
            && !prompt.dropped
        {
            if paste {
                prompt.field.paste(text);
            } else {
                prompt.field.type_in(text);
            }
        }
    }
}

fn layout(text: &str, buttons: usize) -> (Vec<String>, i32) {
    let lines = wrap(text, WRAP);
    let widest = lines.iter().map(|line| FONT.width(line)).max().unwrap_or(0);
    let count = i32::try_from(buttons).unwrap_or(1);
    let row = count * BUTTON.width + (count - 1) * SPACING;
    (lines, ((TEXT + widest + RIGHT) / 2 * 2).max(row + 2 * TEXT))
}

fn wrap(text: &str, width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if FONT.width(candidate.trim_end()) > width && !line.is_empty() {
                lines.push(line.trim_end().to_owned());
                word.clone_into(&mut line);
            } else {
                line = candidate;
            }
        }
        lines.push(line.trim_end().to_owned());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_wrap_like_the_terminate_warning() {
        let text = "WARNING: Terminating a process can cause undesired results including loss of data \
            and system instability. The process will not be given the chance to save its state or data \
            before it is terminated.  Are you sure you want to terminate the process?";
        let (lines, width) = layout(text, 2);
        assert_eq!(width, 342);
        assert_eq!(
            lines,
            [
                "WARNING: Terminating a process can cause undesired",
                "results including loss of data and system instability. The",
                "process will not be given the chance to save its state or",
                "data before it is terminated.  Are you sure you want to",
                "terminate the process?",
            ]
        );
        assert_eq!(
            Dialog::Message(Message::new(Notice::Untouchable)).size(),
            Size::new(278, 126)
        );
    }

    #[test]
    fn select_columns_keeps_image_name() {
        let mut columns = Columns::new(&[Metric::Name, Metric::Pid]);
        columns.toggle(0);
        columns.toggle(1);
        columns.toggle(16);
        columns.toggle(6);
        assert_eq!(columns.chosen(), [Metric::Name, Metric::PeakMemory]);
    }
}
