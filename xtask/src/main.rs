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

mod bundle;

use std::error::Error;
use std::process::ExitCode;

use clap::Parser;

type Failure = Box<dyn Error + Send + Sync>;

#[derive(Parser)]
#[command(bin_name = "cargo xtask", about)]
enum Task {
    #[command(about = "Builds an ad hoc signed Task Manager.app and prints where it is")]
    Bundle {
        #[arg(long, help = "Runs on both Apple silicon and Intel Macs")]
        universal: bool,
    },
}

fn main() -> ExitCode {
    let result = match Task::parse() {
        Task::Bundle { universal } => bundle::bundle(universal),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn command_line_is_well_formed() {
        Task::command().debug_assert();
    }
}
