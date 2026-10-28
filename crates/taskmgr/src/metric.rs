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

use crate::list::Column;
use crate::system::{self, Process, thousands};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Metric {
    Name,
    Pid,
    Cpu,
    CpuTime,
    Memory,
    MemoryDelta,
    PeakMemory,
}

pub(crate) const CHOICES: [(&str, Option<Metric>); 23] = [
    ("Image Name", Some(Metric::Name)),
    ("PID (Process Identifier)", Some(Metric::Pid)),
    ("CPU Usage", Some(Metric::Cpu)),
    ("CPU Time", Some(Metric::CpuTime)),
    ("Memory Usage", Some(Metric::Memory)),
    ("Memory Usage Delta", Some(Metric::MemoryDelta)),
    ("Peak Memory Usage", Some(Metric::PeakMemory)),
    ("Page Faults", None),
    ("USER Objects", None),
    ("I/O Reads", None),
    ("I/O Read Bytes", None),
    ("Page Faults Delta", None),
    ("Virtual Memory Size", None),
    ("Paged Pool", None),
    ("Non-paged Pool", None),
    ("Base Priority", None),
    ("Handle Count", None),
    ("Thread Count", None),
    ("GDI Objects", None),
    ("I/O Writes", None),
    ("I/O Write Bytes", None),
    ("I/O Other", None),
    ("I/O Other Bytes", None),
];

pub(crate) const DEFAULT: [Metric; 5] = [
    Metric::Name,
    Metric::Pid,
    Metric::Cpu,
    Metric::CpuTime,
    Metric::Memory,
];

impl Metric {
    pub(crate) const fn column(self) -> Column {
        let (title, width) = match self {
            Self::Name => ("Image Name", 107),
            Self::Pid => ("PID", 50),
            Self::Cpu => ("CPU", 35),
            Self::CpuTime => ("CPU Time", 70),
            Self::Memory => ("Mem Usage", 70),
            Self::MemoryDelta => ("Mem Delta", 70),
            Self::PeakMemory => ("Peak Mem Usage", 95),
        };
        Column {
            title,
            width,
            numeric: !matches!(self, Self::Name),
        }
    }

    pub(crate) fn cell(self, process: &Process<'_>) -> String {
        let kilobytes = |value: u64| format!("{} K", thousands(value));
        match self {
            Self::Name => process.name.to_owned(),
            Self::Pid => process.pid.to_string(),
            Self::Cpu => format!("{:02} ", process.cpu),
            Self::CpuTime => system::clock(process.seconds),
            Self::Memory => kilobytes(process.memory),
            Self::MemoryDelta => thousands(process.delta),
            Self::PeakMemory => kilobytes(process.peak),
        }
    }
}
