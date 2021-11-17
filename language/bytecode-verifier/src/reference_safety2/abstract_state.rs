// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! This module defines the abstract state for the type and memory safety analysis.
use crate::absint::{AbstractDomain, JoinResult};
use borrow_set::{
    references::RefID,
    set::{Conflicts, QueryFilter},
};
use mirai_annotations::{checked_precondition, checked_verify};
use move_binary_format::{
    binary_views::FunctionView,
    errors::{PartialVMError, PartialVMResult},
    file_format::{
        CodeOffset, FieldHandleIndex, FunctionDefinitionIndex, LocalIndex, Signature,
        SignatureToken, StructDefinitionIndex,
    },
};
use move_core_types::vm_status::StatusCode;
use std::{
    collections::{BTreeMap, BTreeSet},
    iter,
};

type BorrowSet = borrow_set::set::BorrowSet<(), Label>;

/// AbstractValue represents a reference or a non reference value, both on the stack and stored
/// in a local
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AbstractValue {
    Reference(RefID),
    NonReference,
}

impl AbstractValue {
    /// checks if self is a reference
    pub fn is_reference(&self) -> bool {
        match self {
            AbstractValue::Reference(_) => true,
            AbstractValue::NonReference => false,
        }
    }

    /// checks if self is a value
    pub fn is_value(&self) -> bool {
        !self.is_reference()
    }

    /// possibly extracts id from self
    pub fn ref_id(&self) -> Option<RefID> {
        match self {
            AbstractValue::Reference(id) => Some(*id),
            AbstractValue::NonReference => None,
        }
    }
}

/// Label is an element of a label on an edge in the borrow graph.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Label {
    Parameter(LocalIndex),
    Global(StructDefinitionIndex),
    Local(LocalIndex),
    Field(FieldHandleIndex),
}

// Needed for debugging with the borrow graph
impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Label::Parameter(i) => write!(f, "parameter#{}", i),
            Label::Global(i) => write!(f, "resource@{}", i),
            Label::Local(i) => write!(f, "local#{}", i),
            Label::Field(i) => write!(f, "field#{}", i),
        }
    }
}

/// AbstractState is the analysis state over which abstract interpretation is performed.
#[derive(Clone, Debug)]
pub(crate) struct AbstractState {
    current_function: Option<FunctionDefinitionIndex>,
    locals: BTreeMap<LocalIndex, RefID>,
    borrow_set: BorrowSet,
}

impl AbstractState {
    /// create a new abstract state
    pub fn new(function_view: &FunctionView) -> Self {
        let num_params = function_view.parameters().len();
        let param_refs = function_view
            .parameters()
            .0
            .iter()
            .enumerate()
            .filter_map(|(idx, ty)| {
                let mutable = match ty {
                    SignatureToken::MutableReference(_) => true,
                    SignatureToken::Reference(_) => false,
                    _ => return None,
                };
                let idx = idx as LocalIndex;
                Some((
                    idx,
                    mutable,
                    /* Initial ref path */ Some(((), Label::Parameter(idx))),
                ))
            });
        let local_refs = function_view
            .locals()
            .0
            .iter()
            .enumerate()
            .filter_map(|(idx, ty)| {
                let mutable = match ty {
                    SignatureToken::MutableReference(_) => true,
                    SignatureToken::Reference(_) => false,
                    _ => return None,
                };
                let idx = (num_params + idx) as LocalIndex;
                Some((
                    idx, mutable, /* Locals don't start with a value, no initial path */ None,
                ))
            });
        let (borrow_set, locals) = BorrowSet::new(param_refs.chain(local_refs));

        AbstractState {
            current_function: function_view.index(),
            locals,
            borrow_set,
        }
    }

    fn error(&self, status: StatusCode, offset: CodeOffset) -> PartialVMError {
        PartialVMError::new(status).at_code_offset(
            self.current_function.unwrap_or(FunctionDefinitionIndex(0)),
            offset,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn display(&self) {
        self.borrow_set.display();
        println!()
    }

    //**********************************************************************************************
    // Core Predicates
    //**********************************************************************************************

    // Writable if
    // No imm equal
    // No extensions
    fn is_writable(&self, id: RefID) -> bool {
        checked_precondition!(self.borrow_set.is_mutable(id));
        let Conflicts {
            equal: _equal,
            existential: ext_conflicts,
            labeled: lbl_conflicts,
        } = self.borrow_set.borrowed_by(id, QueryFilter::empty());
        ext_conflicts.is_empty() && lbl_conflicts.is_empty()
    }

    fn has_no_parents_in_set(&self, id: RefID, candidates: &BTreeSet<RefID>) -> bool {
        let parents = self
            .borrow_set
            .borrows_from(id, QueryFilter::empty().candidates(candidates));
        parents.is_empty()
    }

    /// checks if local@idx is borrowed
    fn is_local_borrowed(&self, idx: LocalIndex) -> bool {
        let refs = self.borrow_set.all_starting_with_label(&Label::Local(idx));
        !refs.is_empty()
    }

    /// checks if global@idx is borrowed
    fn is_global_borrowed(&self, resource: StructDefinitionIndex) -> bool {
        let refs = self
            .borrow_set
            .all_starting_with_label(&Label::Global(resource));
        !refs.is_empty()
    }

    /// checks if the stack frame of the function being analyzed can be safely destroyed.
    /// safe destruction requires that all references in locals have already been destroyed
    /// and all values in locals are copyable and unborrowed.
    fn is_frame_safe_to_destroy(&self) -> bool {
        let local_or_global_rooted_refs = self
            .borrow_set
            .all_starting_with_predicate(|lbl| matches!(lbl, Label::Global(_) | Label::Local(_)));
        local_or_global_rooted_refs.is_empty()
    }

    //**********************************************************************************************
    // Instruction Entry Points
    //**********************************************************************************************

    /// Releases reference if it is one
    pub fn release_value(&mut self, value: AbstractValue) {
        match value {
            AbstractValue::Reference(id) => self.borrow_set.release(id),
            AbstractValue::NonReference => (),
        }
    }

    pub fn copy_loc(
        &mut self,
        _offset: CodeOffset,
        local: LocalIndex,
    ) -> PartialVMResult<AbstractValue> {
        match self.locals.get(&local) {
            Some(id) => {
                let id = *id;
                let new_id = self
                    .borrow_set
                    .make_copy((), id, self.borrow_set.is_mutable(id));
                Ok(AbstractValue::Reference(new_id))
            }
            None => Ok(AbstractValue::NonReference),
        }
    }

    pub fn move_loc(
        &mut self,
        offset: CodeOffset,
        local: LocalIndex,
    ) -> PartialVMResult<AbstractValue> {
        match self.locals.get(&local) {
            Some(id) => {
                let id = *id;
                let new_id = self
                    .borrow_set
                    .make_copy((), id, self.borrow_set.is_mutable(id));
                self.borrow_set.release(id);
                Ok(AbstractValue::Reference(new_id))
            }
            None if self.is_local_borrowed(local) => {
                Err(self.error(StatusCode::MOVELOC_EXISTS_BORROW_ERROR, offset))
            }
            None => Ok(AbstractValue::NonReference),
        }
    }

    pub fn st_loc(
        &mut self,
        offset: CodeOffset,
        local: LocalIndex,
        new_value: AbstractValue,
    ) -> PartialVMResult<()> {
        match (self.locals.get(&local), new_value) {
            // typing error cases
            (Some(_), AbstractValue::NonReference) | (None, AbstractValue::Reference(_)) => Ok(()),
            // Nonreference case
            (None, AbstractValue::NonReference) if self.is_local_borrowed(local) => {
                Err(self.error(StatusCode::STLOC_UNSAFE_TO_DESTROY_ERROR, offset))
            }
            (None, AbstractValue::NonReference) => Ok(()),
            // Reference case
            (Some(locals_pinned_id), AbstractValue::Reference(new_id)) => {
                self.borrow_set
                    .move_into_pinned((), *locals_pinned_id, new_id);
                Ok(())
            }
        }
    }

    pub fn freeze_ref(&mut self, _offset: CodeOffset, id: RefID) -> PartialVMResult<AbstractValue> {
        let frozen_id = self.borrow_set.make_copy((), id, false);
        self.borrow_set.release(id);
        Ok(AbstractValue::Reference(frozen_id))
    }

    pub fn read_ref(&mut self, _offset: CodeOffset, id: RefID) -> PartialVMResult<AbstractValue> {
        self.borrow_set.release(id);
        Ok(AbstractValue::NonReference)
    }

    pub fn comparison(
        &mut self,
        _offset: CodeOffset,
        v1: AbstractValue,
        v2: AbstractValue,
    ) -> PartialVMResult<AbstractValue> {
        match (v1, v2) {
            (AbstractValue::Reference(id1), AbstractValue::Reference(id2)) => {
                self.borrow_set.release(id1);
                self.borrow_set.release(id2)
            }
            (v1, v2) => {
                checked_verify!(v1.is_value());
                checked_verify!(v2.is_value());
            }
        }
        Ok(AbstractValue::NonReference)
    }

    pub fn write_ref(&mut self, offset: CodeOffset, id: RefID) -> PartialVMResult<()> {
        if !self.is_writable(id) {
            return Err(self.error(StatusCode::WRITEREF_EXISTS_BORROW_ERROR, offset));
        }

        self.borrow_set.release(id);
        Ok(())
    }

    pub fn borrow_loc(
        &mut self,
        _offset: CodeOffset,
        mut_: bool,
        local: LocalIndex,
    ) -> PartialVMResult<AbstractValue> {
        let new_id = self
            .borrow_set
            .extend_by_label(iter::empty(), (), mut_, Label::Local(local));
        Ok(AbstractValue::Reference(new_id))
    }

    pub fn borrow_field(
        &mut self,
        _offset: CodeOffset,
        mut_: bool,
        id: RefID,
        field: FieldHandleIndex,
    ) -> PartialVMResult<AbstractValue> {
        let new_id = self
            .borrow_set
            .extend_by_label(id, (), mut_, Label::Field(field));
        self.borrow_set.release(id);
        Ok(AbstractValue::Reference(new_id))
    }

    pub fn vector_element_borrow(
        &mut self,
        _offset: CodeOffset,
        mut_: bool,
        id: RefID,
    ) -> PartialVMResult<AbstractValue> {
        let new_id = self.borrow_set.extend_by_unknown(id, (), mut_);
        self.borrow_set.release(id);
        Ok(AbstractValue::Reference(new_id))
    }

    pub fn borrow_global(
        &mut self,
        _offset: CodeOffset,
        mut_: bool,
        resource: StructDefinitionIndex,
    ) -> PartialVMResult<AbstractValue> {
        let new_id =
            self.borrow_set
                .extend_by_label(iter::empty(), (), mut_, Label::Global(resource));
        Ok(AbstractValue::Reference(new_id))
    }

    pub fn move_from(
        &mut self,
        offset: CodeOffset,
        resource: StructDefinitionIndex,
    ) -> PartialVMResult<AbstractValue> {
        if self.is_global_borrowed(resource) {
            Err(self.error(StatusCode::GLOBAL_REFERENCE_ERROR, offset))
        } else {
            Ok(AbstractValue::NonReference)
        }
    }

    pub fn vector_mut_op(&mut self, offset: CodeOffset, id: RefID) -> PartialVMResult<()> {
        if !self.is_writable(id) {
            return Err(self.error(StatusCode::VEC_UPDATE_EXISTS_MUTABLE_BORROW_ERROR, offset));
        }
        self.borrow_set.release(id);
        Ok(())
    }

    pub fn call(
        &mut self,
        offset: CodeOffset,
        arguments: Vec<AbstractValue>,
        acquired_resources: &BTreeSet<StructDefinitionIndex>,
        return_: &Signature,
    ) -> PartialVMResult<Vec<AbstractValue>> {
        // Check acquires
        for acquired_resource in acquired_resources {
            if self.is_global_borrowed(*acquired_resource) {
                return Err(self.error(StatusCode::GLOBAL_REFERENCE_ERROR, offset));
            }
        }

        // Check mutable references can be transfered
        let all_references_to_borrow_from = arguments
            .iter()
            .filter_map(|v| v.ref_id())
            .collect::<BTreeSet<_>>();
        let mut mutable_references_to_borrow_from = BTreeSet::new();
        for id in all_references_to_borrow_from
            .iter()
            .filter(|id| self.borrow_set.is_mutable(**id))
            .copied()
        {
            let is_transferable = self.is_writable(id)
                && self.has_no_parents_in_set(id, &all_references_to_borrow_from);
            if !is_transferable {
                return Err(self.error(StatusCode::CALL_BORROWED_MUTABLE_REFERENCE_ERROR, offset));
            }
            mutable_references_to_borrow_from.insert(id);
        }

        // Track borrow relationships of return values on inputs
        let return_values = return_
            .0
            .iter()
            .map(|return_type| match return_type {
                SignatureToken::MutableReference(_) => {
                    let id = self.borrow_set.extend_by_unknown(
                        mutable_references_to_borrow_from.iter().copied(),
                        (),
                        true,
                    );
                    AbstractValue::Reference(id)
                }
                SignatureToken::Reference(_) => {
                    let id = self.borrow_set.extend_by_unknown(
                        all_references_to_borrow_from.iter().copied(),
                        (),
                        true,
                    );
                    AbstractValue::Reference(id)
                }
                _ => AbstractValue::NonReference,
            })
            .collect();

        // Release input references
        for id in all_references_to_borrow_from {
            self.borrow_set.release(id)
        }
        Ok(return_values)
    }

    pub fn ret(&mut self, offset: CodeOffset, values: Vec<AbstractValue>) -> PartialVMResult<()> {
        // release all local variables
        for pinned_id in self.locals.values().copied() {
            if !self.borrow_set.is_pinned_released(pinned_id) {
                self.borrow_set.release(pinned_id);
            }
        }

        // Check that no local or global is borrowed
        if !self.is_frame_safe_to_destroy() {
            return Err(self.error(
                StatusCode::UNSAFE_RET_LOCAL_OR_RESOURCE_STILL_BORROWED,
                offset,
            ));
        }

        // Check mutable references can be transfered
        let all_return_refs = values
            .into_iter()
            .filter_map(|v| v.ref_id())
            .collect::<BTreeSet<_>>();
        let mutable_return_refs = all_return_refs
            .iter()
            .filter(|id| self.borrow_set.is_mutable(**id))
            .copied();
        for id in mutable_return_refs {
            let is_transferable =
                self.is_writable(id) && self.has_no_parents_in_set(id, &all_return_refs);
            if !is_transferable {
                return Err(self.error(StatusCode::RET_BORROWED_MUTABLE_REFERENCE_ERROR, offset));
            }
        }
        Ok(())
    }

    //**********************************************************************************************
    // Abstract Interpreter Entry Points
    //**********************************************************************************************

    pub fn join_(&self, other: &Self) -> Self {
        checked_precondition!(self.current_function == other.current_function);
        checked_precondition!(self.locals == other.locals);
        let mut self_set = self.borrow_set.clone();
        let mut other_set = other.borrow_set.clone();
        for ref_id in self.locals.values().copied() {
            match (
                self_set.is_pinned_released(ref_id),
                other_set.is_pinned_released(ref_id),
            ) {
                // Released on both sides
                (true, true) => (),

                (false, true) => {
                    // A reference exists on one side, but not the other. Release
                    self_set.release(ref_id);
                }
                (true, false) => {
                    // A reference exists on one side, but not the other. Release
                    other_set.release(ref_id);
                }

                // Reference is bound on both sides
                (false, false) => (),
            }
        }

        let current_function = self.current_function;
        let locals = self.locals.clone();
        let borrow_set = self_set.join(&other_set);

        Self {
            current_function,
            locals,
            borrow_set,
        }
    }
}

impl AbstractDomain for AbstractState {
    /// attempts to join state to self and returns the result
    fn join(&mut self, state: &AbstractState) -> JoinResult {
        let joined = Self::join_(self, state);
        checked_verify!(self.current_function == joined.current_function);
        checked_verify!(self.locals == joined.locals);
        let locals_changed = self.locals.values().copied().any(|id| {
            self.borrow_set.is_pinned_released(id) != joined.borrow_set.is_pinned_released(id)
        });
        if !locals_changed && self.borrow_set.covers(&joined.borrow_set) {
            JoinResult::Unchanged
        } else {
            *self = joined;
            JoinResult::Changed
        }
    }
}
