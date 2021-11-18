// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use move_bytecode_verifier::CodeUnitVerifier;
use std::path::Path;
use structopt::*;

#[derive(Debug, StructOpt)]
#[structopt(name = "Anayalze", about = "Print stuff for paper")]
pub struct Options {
    /// The source files to check
    #[structopt(name = "PATH_TO_SOURCE_FILE")]
    pub source_file: String,

    #[structopt(name = "MESSAGE", short = "m", long = "message")]
    pub msg: String,
    #[structopt(name = "ITERATIONS", short = "n", long = "iterations")]
    pub iterations: u128,
    #[structopt(name = "SANITY CHECK", long = "sanity")]
    pub sanity_check: bool,
}

pub fn main() -> anyhow::Result<()> {
    let Options {
        source_file,
        msg,
        iterations,
        sanity_check,
    } = Options::from_args();
    let (module, _) = ir_compiler::util::do_compile_module(Path::new(&source_file), &[]);
    if sanity_check {
        move_bytecode_verifier::verify_module(&module).unwrap();
        println!("sanity check passed")
    }
    assert!(iterations > 0);
    let now = std::time::Instant::now();
    for _ in 0..iterations {
        CodeUnitVerifier::verify_module(&module).unwrap();
    }
    let _elapsed = now.elapsed().as_millis();
    let time = _elapsed / iterations;
    println!("{}, {} ms", msg, time);
    Ok(())
}
