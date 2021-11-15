// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use move_binary_format::file_format::*;
use move_core_types::language_storage::ModuleId;
use move_lang::{
    command_line::{self as cli},
    compiled_unit::{
        self, AnnotatedCompiledModule, AnnotatedCompiledScript, AnnotatedCompiledUnit,
        NamedCompiledModule, NamedCompiledScript,
    },
    diagnostics::codes::Severity,
    shared::NumericalAddress,
};
use std::{cmp::max, collections::BTreeSet};
use structopt::*;

#[derive(Debug, StructOpt)]
#[structopt(name = "Anayalze", about = "Print stuff for paper")]
pub struct Options {
    /// The source files to check
    #[structopt(name = "PATH_TO_SOURCE_FILE")]
    pub source_files: Vec<String>,

    /// The library files needed as dependencies
    #[structopt(
        name = "PATH_TO_DEPENDENCY_FILE",
        short = cli::DEPENDENCY_SHORT,
        long = cli::DEPENDENCY,
    )]
    pub dependencies: Vec<String>,
}

pub fn main() -> anyhow::Result<()> {
    let Options {
        source_files,
        dependencies,
    } = Options::from_args();
    let mapping = [
        ("Std", "0x1"),
        ("DiemFramework", "0x1"),
        ("DiemRoot", "0xA550C18"),
        ("CurrencyInfo", "0xA550C18"),
        ("TreasuryCompliance", "0xB1E55ED"),
        ("VMReserved", "0x0"),
    ];
    let address_map = mapping
        .iter()
        .map(|(name, addr)| (name.to_string(), NumericalAddress::parse_str(addr).unwrap()))
        .collect();

    let (files, compiled_units_res) = move_lang::Compiler::new(&source_files, &dependencies)
        .set_named_address_values(address_map)
        .build()?;
    let (compiled_units, diags) = match compiled_units_res {
        Ok(res) => res,
        Err(diags) => move_lang::diagnostics::report_diagnostics(&files, diags),
    };
    if diags.max_severity().unwrap_or(Severity::Warning) > Severity::Warning {
        move_lang::diagnostics::report_diagnostics(&files, diags)
    }
    const ITERATIONS: u128 = if cfg!(debug_assertions) { 10 } else { 100 };
    let now = std::time::Instant::now();
    for _ in 0..ITERATIONS {
        let diags = compiled_unit::verify_units(&compiled_units);
        if !diags.is_empty() {
            move_lang::diagnostics::report_diagnostics(&files, diags)
        }
    }
    println!(
        "\
##################################################################
{}
    ",
        source_files.join("\n")
    );
    let _elapsed = now.elapsed().as_millis();

    let mut counts = Counts::default();
    for unit in &compiled_units {
        match unit {
            AnnotatedCompiledUnit::Script(AnnotatedCompiledScript {
                named_script: NamedCompiledScript { script, .. },
                ..
            }) => count_script(&mut counts, script),
            AnnotatedCompiledUnit::Module(AnnotatedCompiledModule {
                named_module: NamedCompiledModule { module, .. },
                ..
            }) => count_module(&mut counts, module),
        }
    }

    println!(
        "Total milliseconds to verify compiled units after {} iterations: {}",
        ITERATIONS, _elapsed
    );
    println!(
        "Average milliseconds to verify compiled units after {} iterations: {}",
        ITERATIONS,
        (_elapsed as f64) / (ITERATIONS as f64)
    );

    println!(
        "Bytecode instructions analyzed per millisecond: ~{}",
        (ITERATIONS * (counts.total_instructions as u128)) / (_elapsed),
    );
    counts.print();
    Ok(())
}

#[derive(Default)]
struct Counts {
    num_scripts: usize,
    imm_borrow_loc: usize,
    mut_borrow_loc: usize,
    imm_borrow_field: usize,
    mut_borrow_field: usize,
    imm_borrow_global: usize,
    mut_borrow_global: usize,
    exists: usize,
    move_from: usize,
    move_to: usize,
    freeze: usize,
    total_instructions: usize,
    max_locals: usize,

    reference_parameters: usize,
    reference_return_values: usize,
    acquires_annotations: usize,

    total_functions: usize,
    functions_with_global_storage_operations: usize,
    functions_with_reference_operations: usize,
    functions_with_reference_signatures: usize,
    function_set_with_reference_sig_or_op: BTreeSet<(Option<ModuleId>, FunctionDefinitionIndex)>,
    functions_with_acquires: usize,

    total_structs: usize,
    total_fields: usize,
    max_structs: usize,
    max_fields: usize,

    total_modules: usize,
    modules_with_acquires: usize,
}

impl Counts {
    fn print(self) {
        macro_rules! percent {
            ($x:expr, $y:expr) => {{
                let x = $x;
                let y = $y;
                format!("{}/{} ({:.2}%)", x, y, (x as f64) / (y as f64) * 100.)
            }};
        }

        let total_reference_operations = self.total_reference_operations();
        let Counts {
            num_scripts: _num_scripts,
            imm_borrow_loc,
            mut_borrow_loc,
            imm_borrow_field,
            mut_borrow_field,
            imm_borrow_global,
            mut_borrow_global,
            exists,
            move_from,
            move_to,
            freeze,
            total_instructions,
            max_locals,
            reference_parameters,
            reference_return_values,
            acquires_annotations,
            total_functions,
            functions_with_global_storage_operations,
            functions_with_reference_operations,
            functions_with_reference_signatures,
            function_set_with_reference_sig_or_op,
            functions_with_acquires,
            total_structs,
            total_fields,
            max_structs,
            max_fields,
            total_modules,
            modules_with_acquires,
        } = self;
        let functions_with_reference_sig_or_op = function_set_with_reference_sig_or_op.len();
        println!(
            "Total reference operations (not including move/copy/pop): {}",
            total_reference_operations
        );
        println!("  Total borrow local: {}", imm_borrow_loc + mut_borrow_loc);
        println!("    Imm borrow local: {}", imm_borrow_loc);
        println!("    Mut borrow local: {}", mut_borrow_loc);
        println!(
            "  Total borrow field: {}",
            imm_borrow_field + mut_borrow_field
        );
        println!("    Imm borrow field: {}", imm_borrow_field);
        println!("    Mut borrow field: {}", mut_borrow_field);
        println!(
            "  Total borrow global: {}",
            imm_borrow_global + mut_borrow_global
        );
        println!("    Imm borrow global: {}", imm_borrow_global);
        println!("    Mut borrow global: {}", mut_borrow_global);
        println!("  Freeze: {}", freeze);
        println!(
            "Fraction of instructions that are reference instructions: {}",
            percent!(total_reference_operations, total_instructions)
        );
        println!("Exists: {}", exists);
        println!("Move from: {}", move_from);
        println!("Move from: {}", move_to);
        println!("Max number of locals in a given function: {}", max_locals);
        println!();

        let total_annots = reference_parameters + reference_return_values + acquires_annotations;
        println!("Total reference related annotations: {}", total_annots);
        println!(
            "  Total reference function type annotations: {}",
            reference_parameters + reference_return_values
        );
        println!("    Reference parameters: {}", reference_parameters);
        println!("    Reference return values: {}", reference_return_values);
        println!("  Acquire annotations: {}", acquires_annotations);
        println!();

        println!(
            "Functions with global storage operations: {}",
            percent!(functions_with_global_storage_operations, total_functions)
        );
        println!(
            "Functions with reference operations: {}",
            percent!(functions_with_reference_operations, total_functions)
        );
        println!(
            "Functions with reference signatures: {}",
            percent!(functions_with_reference_signatures, total_functions)
        );
        println!(
            "Functions with reference operations or reference signatures: {}",
            percent!(functions_with_reference_sig_or_op, total_functions)
        );
        println!(
            "Functions with global storage operations: {}",
            percent!(functions_with_acquires, total_functions)
        );
        println!(
            "Functions with acquires: {}",
            percent!(functions_with_acquires, total_functions)
        );
        println!(
            "Modules with acquires: {}",
            percent!(modules_with_acquires, total_modules)
        );
        println!("Total number of structs: {}", total_structs);
        println!("Total number of fields: {}", total_fields);
        println!("Max number of structs in a single module: {}", max_structs);
        println!("Max number of fields in a single struct: {}", max_fields);
        println!();
    }

    fn total_reference_operations(&self) -> usize {
        self.imm_borrow_loc
            + self.mut_borrow_loc
            + self.imm_borrow_field
            + self.mut_borrow_field
            + self.imm_borrow_global
            + self.mut_borrow_global
            + self.freeze
    }

    fn total_global_operations(&self) -> usize {
        self.imm_borrow_global
            + self.mut_borrow_global
            + self.move_from
            + self.move_to
            + self.exists
    }
}

fn count_module(counts: &mut Counts, module: &CompiledModule) {
    counts.total_modules += 1;
    let before_acquires = counts.acquires_annotations;
    let id = module.self_id();
    // structs
    let mut num_structs = 0;
    let mut num_fields = 0;
    for sdef in &module.struct_defs {
        let is_generic = !module.struct_handles[sdef.struct_handle.0 as usize]
            .type_parameters
            .is_empty();
        let structs_fields = match &sdef.field_information {
            StructFieldInformation::Native => 0,
            StructFieldInformation::Declared(fields) => fields.len(),
        };
        counts.max_fields = max(counts.max_fields, structs_fields);
        if is_generic {
            continue;
        }
        num_structs += 1;
        num_fields += structs_fields;
    }
    for sdef_inst in &module.struct_def_instantiations {
        assert!(!module.signatures[sdef_inst.type_parameters.0 as usize]
            .0
            .is_empty());
    }
    num_structs += module.struct_def_instantiations.len();
    for field_inst in &module.field_instantiations {
        assert!(!module.signatures[field_inst.type_parameters.0 as usize]
            .0
            .is_empty());
    }
    num_fields += module.field_instantiations.len();

    counts.total_structs += num_structs;
    counts.total_fields += num_fields;
    counts.max_structs = max(num_structs, counts.max_structs);

    // functions
    for (idx, fdef) in module.function_defs.iter().enumerate() {
        let fhandle = &module.function_handles[fdef.function.0 as usize];
        let empty = vec![];
        let locals = fdef
            .code
            .as_ref()
            .map(|code| &module.signatures[code.locals.0 as usize].0)
            .unwrap_or(&empty);
        count_function_signature(
            counts,
            Some(id.clone()),
            FunctionDefinitionIndex(idx as TableIndex),
            &module.signatures[fhandle.parameters.0 as usize].0,
            &module.signatures[fhandle.return_.0 as usize].0,
            &fdef.acquires_global_resources,
            &locals,
        );
        if let Some(code) = &fdef.code {
            count_instructions(
                counts,
                Some(id.clone()),
                FunctionDefinitionIndex(idx as TableIndex),
                &code.code,
            )
        }
    }
    let after_acquires = counts.acquires_annotations;
    if after_acquires > before_acquires {
        counts.modules_with_acquires += 1;
    }
}

fn count_script(counts: &mut Counts, script: &CompiledScript) {
    counts.num_scripts += 1;
    count_function_signature(
        counts,
        None,
        FunctionDefinitionIndex(counts.num_scripts as TableIndex),
        &script.signatures[script.parameters.0 as usize].0,
        &vec![],
        &vec![],
        &script.signatures[script.code.locals.0 as usize].0,
    );
    count_instructions(
        counts,
        None,
        FunctionDefinitionIndex(counts.num_scripts as TableIndex),
        &script.code.code,
    )
}

fn count_function_signature(
    counts: &mut Counts,
    module: Option<ModuleId>,
    def: FunctionDefinitionIndex,
    parameters: &[SignatureToken],
    return_types: &[SignatureToken],
    acquires: &[StructDefinitionIndex],
    locals: &[SignatureToken],
) {
    counts.total_functions += 1;
    let mut has_reference = false;
    for parameter in parameters {
        match parameter {
            SignatureToken::Reference(_) | SignatureToken::MutableReference(_) => {
                has_reference = true;
                counts.reference_parameters += 1
            }
            _ => (),
        }
    }
    for return_type in return_types {
        match return_type {
            SignatureToken::Reference(_) | SignatureToken::MutableReference(_) => {
                has_reference = true;
                counts.reference_return_values += 1
            }
            _ => (),
        }
    }
    if has_reference {
        counts
            .function_set_with_reference_sig_or_op
            .insert((module, def));
        counts.functions_with_reference_signatures += 1;
    }
    if !acquires.is_empty() {
        counts.functions_with_acquires += 1;
    }
    counts.acquires_annotations += acquires.len();

    let num_locals = parameters.len() + locals.len();
    counts.max_locals = max(counts.max_locals, num_locals);
}

fn count_instructions(
    counts: &mut Counts,
    module: Option<ModuleId>,
    def: FunctionDefinitionIndex,
    code: &[Bytecode],
) {
    let before_gso = counts.total_global_operations();
    let before_reference_instruction = counts.total_reference_operations();
    for instr in code {
        count_instruction(counts, instr)
    }
    let after_gso = counts.total_global_operations();
    if after_gso > before_gso {
        counts.functions_with_global_storage_operations += 1;
    }
    let after_reference_instruction = counts.total_reference_operations();
    if after_reference_instruction > before_reference_instruction {
        counts
            .function_set_with_reference_sig_or_op
            .insert((module, def));
        counts.functions_with_reference_operations += 1;
    }
}

fn count_instruction(counts: &mut Counts, instr: &Bytecode) {
    counts.total_instructions += 1;
    match instr {
        Bytecode::ImmBorrowLoc(_) => counts.imm_borrow_loc += 1,
        Bytecode::MutBorrowLoc(_) => counts.mut_borrow_loc += 1,

        Bytecode::ImmBorrowField(_) | Bytecode::ImmBorrowFieldGeneric(_) => {
            counts.imm_borrow_field += 1
        }
        Bytecode::MutBorrowField(_) | Bytecode::MutBorrowFieldGeneric(_) => {
            counts.mut_borrow_field += 1
        }

        Bytecode::ImmBorrowGlobal(_) | Bytecode::ImmBorrowGlobalGeneric(_) => {
            counts.imm_borrow_global += 1
        }
        Bytecode::MutBorrowGlobal(_) | Bytecode::MutBorrowGlobalGeneric(_) => {
            counts.mut_borrow_global += 1
        }

        Bytecode::Exists(_) | Bytecode::ExistsGeneric(_) => counts.exists += 1,
        Bytecode::MoveFrom(_) | Bytecode::MoveFromGeneric(_) => counts.move_from += 1,
        Bytecode::MoveTo(_) | Bytecode::MoveToGeneric(_) => counts.move_to += 1,

        Bytecode::FreezeRef => counts.freeze += 1,

        _ => (),
    }
}
