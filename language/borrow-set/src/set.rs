// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    map::{merge_btree_map, BorrowMap},
    references::*,
};
use mirai_annotations::{
    debug_checked_postcondition, debug_checked_precondition, debug_checked_verify,
};
use std::collections::BTreeMap;

//**************************************************************************************************
// Definitions
//**************************************************************************************************

pub use crate::map::{Conflicts, Parents, QueryFilter};

#[derive(Clone, Debug, Default)]
pub struct BorrowSet<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> {
    worlds: Vec<BorrowMap<Loc, Instr, Lbl>>,
}

//**************************************************************************************************
// impls
//**************************************************************************************************

impl<Loc: Copy, Instr: Copy + Ord + std::fmt::Display, Lbl: Clone + Ord + std::fmt::Display>
    BorrowSet<Loc, Instr, Lbl>
{
    pub fn new<K: Ord>(
        pinned_initial_refs: impl IntoIterator<Item = (K, bool, Option<(Loc, Lbl)>)>,
    ) -> (Self, BTreeMap<K, RefID>) {
        let (map, ids) = BorrowMap::new(pinned_initial_refs);
        (Self { worlds: vec![map] }, ids)
    }

    //**********************************************************************************************
    // Ref API
    //**********************************************************************************************

    /// checks if the given reference is mutable or not
    pub fn is_mutable(&self, id: RefID) -> bool {
        debug_checked_precondition!(self.satisfies_invariant());
        self.worlds[0].is_mutable(id)
    }

    pub fn make_copy(&mut self, loc: Loc, id: RefID, mutable: bool) -> RefID {
        debug_checked_precondition!(self.satisfies_invariant());
        let mut worlds = self.worlds.iter_mut();
        let new_id = worlds.next().unwrap().make_copy(loc, id, mutable);
        for world in worlds {
            let other_id = world.make_copy(loc, id, mutable);
            debug_checked_verify!(new_id == other_id)
        }
        new_id
    }

    pub fn extend_by_label(
        &mut self,
        sources: impl IntoIterator<Item = RefID>,
        loc: Loc,
        mutable: bool,
        lbl: Lbl,
    ) -> RefID {
        debug_checked_precondition!(self.satisfies_invariant());
        let sources = sources.into_iter().collect::<Vec<_>>();
        let mut worlds = self.worlds.iter_mut();
        let new_id =
            worlds
                .next()
                .unwrap()
                .extend_by_label(sources.clone(), loc, mutable, lbl.clone());
        for world in worlds {
            let other_id = world.extend_by_label(sources.clone(), loc, mutable, lbl.clone());
            debug_checked_verify!(new_id == other_id)
        }
        new_id
    }

    pub fn extend_by_unknown(
        &mut self,
        sources: impl IntoIterator<Item = RefID>,
        loc: Loc,
        mutable: bool,
        instr: Instr,
        ref_created_at_instr: usize,
    ) -> RefID {
        debug_checked_precondition!(self.satisfies_invariant());
        let sources = sources.into_iter().collect::<Vec<_>>();
        let mut worlds = self.worlds.iter_mut();
        let new_id = worlds.next().unwrap().extend_by_unknown(
            sources.clone(),
            loc,
            mutable,
            instr,
            ref_created_at_instr,
        );
        for world in worlds {
            let other_id =
                world.extend_by_unknown(sources.clone(), loc, mutable, instr, ref_created_at_instr);
            debug_checked_verify!(new_id == other_id)
        }
        new_id
    }

    pub fn move_into_pinned(&mut self, loc: Loc, pinned: RefID, other: RefID) {
        debug_checked_precondition!(self.satisfies_invariant());
        for world in &mut self.worlds {
            world.move_into_pinned(loc, pinned, other)
        }
    }

    pub fn release(&mut self, id: RefID) {
        debug_checked_precondition!(self.satisfies_invariant());
        for world in &mut self.worlds {
            world.release(id)
        }
    }

    //**********************************************************************************************
    // Query API
    //**********************************************************************************************

    pub fn borrowed_by(&self, id: RefID, filter: QueryFilter) -> Conflicts<Loc, Lbl> {
        debug_checked_precondition!(self.satisfies_invariant());
        self.worlds
            .iter()
            .map(|world| world.borrowed_by(id, filter.clone()))
            .reduce(|conflicts1, conflicts2| conflicts1.merge(conflicts2))
            .unwrap()
    }

    pub fn borrows_from(&self, id: RefID, filter: QueryFilter) -> Parents<Loc, Lbl> {
        debug_checked_precondition!(self.satisfies_invariant());
        self.worlds
            .iter()
            .map(|world| world.borrows_from(id, filter.clone()))
            .reduce(|parents1, parents2| parents1.merge(parents2))
            .unwrap()
    }

    pub fn all_starting_with_label(&self, lbl: &Lbl) -> BTreeMap<RefID, Loc> {
        debug_checked_precondition!(self.satisfies_invariant());
        self.worlds
            .iter()
            .map(|world| world.all_starting_with_label(lbl))
            .reduce(|refs1, refs2| merge_btree_map(refs1, refs2))
            .unwrap()
    }

    pub fn all_starting_with_predicate(
        &self,
        mut p: impl FnMut(&Lbl) -> bool,
    ) -> BTreeMap<RefID, Loc> {
        debug_checked_precondition!(self.satisfies_invariant());
        self.worlds
            .iter()
            .map(|world| world.all_starting_with_predicate(&mut p))
            .reduce(|refs1, refs2| merge_btree_map(refs1, refs2))
            .unwrap()
    }

    /// Returns true iff a pinned id has no borrows
    pub fn is_pinned_released(&self, id: RefID) -> bool {
        debug_checked_precondition!(self.satisfies_invariant());
        self.worlds[0].is_pinned_released(id)
    }

    //**********************************************************************************************
    // Joining
    //**********************************************************************************************

    pub fn covers(&self, other: &Self) -> bool {
        other.worlds.iter().all(|other_world| {
            self.worlds
                .iter()
                .any(|self_world| self_world.covers(other_world))
        })
    }

    pub fn join(&self, other: &Self) -> Self {
        debug_checked_precondition!(self.satisfies_invariant());
        debug_checked_precondition!(other.satisfies_invariant());
        let mut joined = self.clone();
        let not_covered_worlds = other
            .worlds
            .iter()
            .filter(|other_world| {
                !self
                    .worlds
                    .iter()
                    .any(|self_world| self_world.covers(other_world))
            })
            .cloned();
        joined.worlds.extend(not_covered_worlds);
        for world in &mut joined.worlds {
            world.set_next_id_after_join()
        }
        debug_checked_postcondition!(joined.satisfies_invariant());
        joined
    }

    //**********************************************************************************************
    // Invariants
    //**********************************************************************************************

    pub fn satisfies_invariant(&self) -> bool {
        if self.worlds.is_empty() {
            return false;
        }
        let world_0 = &self.worlds[0];
        let mut other_worlds = self.worlds.iter().skip(1);
        other_worlds.all(|other| world_0.consistent_world(other))
    }

    //**********************************************************************************************
    // Util
    //**********************************************************************************************

    #[allow(dead_code)]
    pub fn display(&self)
    where
        Instr: std::fmt::Display,
        Lbl: std::fmt::Display,
    {
        for world in &self.worlds {
            println!("{{");
            world.display();
            println!("}},");
        }
    }
}
