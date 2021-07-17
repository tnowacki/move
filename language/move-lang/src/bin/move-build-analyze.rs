// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use move_lang::{
    command_line::{self as cli},
    shared::Flags,
};
use structopt::*;

#[derive(Debug, StructOpt)]
#[structopt(name = "Move Build", about = "Compile Move source to Move bytecode.")]
pub struct Options {
    /// The source files to check and compile
    #[structopt(name = "PATH_TO_SOURCE_FILE")]
    pub source_files: Vec<String>,
}

fn do_once(source_files: &[String]) -> anyhow::Result<()> {
    let interface_files_dir = format!("{}/generated_interface_files", cli::DEFAULT_OUTPUT_DIR);
    let (files, compiled_units) = move_lang::Compiler::new(&source_files, &[])
        .set_interface_files_dir(interface_files_dir)
        .set_flags(Flags::empty())
        .build_and_report()?;
    move_lang::output_compiled_units(false, files, compiled_units, cli::DEFAULT_OUTPUT_DIR)
}

pub fn main() -> anyhow::Result<()> {
    let Options { source_files } = Options::from_args();
    const ITERATIONS: u128 = if cfg!(debug_assertions) { 10 } else { 100 };
    let now = std::time::Instant::now();
    for _ in 0..ITERATIONS {
        do_once(&source_files)?
    }
    let _elapsed = now.elapsed().as_millis();
    println!(
        "Total miliseconds to verify compiled units after {} iterations: {}",
        ITERATIONS, _elapsed
    );
    println!(
        "Average miliseconds to verify compiled units after {} iterations: {}",
        ITERATIONS,
        (_elapsed as f64) / (ITERATIONS as f64)
    );
    Ok(())
}
