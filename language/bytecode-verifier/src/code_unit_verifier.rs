// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! This module implements the checker for verifying correctness of function bodies.
//! The overall verification is split between stack_usage_verifier.rs and
//! abstract_interpreter.rs. CodeUnitVerifier simply orchestrates calls into these two files.
use crate::{
    acquires_list_verifier::AcquiresVerifier, control_flow, locals_safety, reference_safety,
    reference_safety2, stack_usage_verifier::StackUsageVerifier, type_safety,
};
use move_binary_format::{
    access::ModuleAccess,
    binary_views::{BinaryIndexedView, FunctionView},
    errors::{Location, PartialVMResult, VMResult},
    file_format::{
        CompiledModule, CompiledScript, FunctionDefinition, FunctionDefinitionIndex,
        IdentifierIndex, TableIndex,
    },
    IndexKind,
};
use std::collections::HashMap;

pub struct CodeUnitVerifier<'a> {
    resolver: BinaryIndexedView<'a>,
    function_view: FunctionView<'a>,
    name_def_map: HashMap<IdentifierIndex, FunctionDefinitionIndex>,
}

impl<'a> CodeUnitVerifier<'a> {
    pub fn verify_module(module: &'a CompiledModule) -> VMResult<()> {
        Self::verify_module_impl(module).map_err(|e| e.finish(Location::Module(module.self_id())))
    }

    fn verify_module_impl(module: &'a CompiledModule) -> PartialVMResult<()> {
        for (idx, function_definition) in module.function_defs().iter().enumerate() {
            let index = FunctionDefinitionIndex(idx as TableIndex);
            Self::verify_function(index, function_definition, module)
                .map_err(|err| err.at_index(IndexKind::FunctionDefinition, index.0))?
        }
        Ok(())
    }

    pub fn verify_script(module: &'a CompiledScript) -> VMResult<()> {
        Self::verify_script_impl(module).map_err(|e| e.finish(Location::Script))
    }

    fn verify_script_impl(script: &'a CompiledScript) -> PartialVMResult<()> {
        // create `FunctionView` and `BinaryIndexedView`
        let function_view = FunctionView::script(script);
        let resolver = BinaryIndexedView::Script(script);
        //verify
        let code_unit_verifier = CodeUnitVerifier {
            resolver,
            function_view,
            name_def_map: HashMap::new(),
        };
        code_unit_verifier.verify_common()
    }

    fn verify_function(
        index: FunctionDefinitionIndex,
        function_definition: &'a FunctionDefinition,
        module: &'a CompiledModule,
    ) -> PartialVMResult<()> {
        // nothing to verify for native function
        let code = match &function_definition.code {
            Some(code) => code,
            None => return Ok(()),
        };
        // create `FunctionView` and `BinaryIndexedView`
        let function_handle = module.function_handle_at(function_definition.function);
        let function_view = FunctionView::function(module, index, code, function_handle);
        let resolver = BinaryIndexedView::Module(module);
        let mut name_def_map = HashMap::new();
        for (idx, func_def) in module.function_defs().iter().enumerate() {
            let fh = module.function_handle_at(func_def.function);
            name_def_map.insert(fh.name, FunctionDefinitionIndex(idx as u16));
        }
        // verify
        let code_unit_verifier = CodeUnitVerifier {
            resolver,
            function_view,
            name_def_map,
        };
        code_unit_verifier.verify_common()?;
        AcquiresVerifier::verify(module, index, function_definition)
    }

    fn verify_common(&self) -> PartialVMResult<()> {
        const REF_NEW_VERSION_REMOVE_ME: &'static str = "SET_BASED";
        let use_new_ref_safety = {
            let val = std::env::var(REF_NEW_VERSION_REMOVE_ME).expect(&format!(
                "Please set the env '{}' to 'true'/'1' or 'false'/'0'. \
                Any non-true setting will be interpreted as false",
                REF_NEW_VERSION_REMOVE_ME
            ));
            val.parse::<bool>() == Ok(true) || val.parse::<usize>() == Ok(1)
        };
        control_flow::verify(self.function_view.index(), self.function_view.code())?;
        StackUsageVerifier::verify(&self.resolver, &self.function_view)?;
        type_safety::verify(&self.resolver, &self.function_view)?;
        locals_safety::verify(&self.resolver, &self.function_view)?;
        if use_new_ref_safety {
            reference_safety2::verify(&self.resolver, &self.function_view, &self.name_def_map)?
        } else {
            reference_safety::verify(&self.resolver, &self.function_view, &self.name_def_map)?
        };
        Ok(())
    }
}
