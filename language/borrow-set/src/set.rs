// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{paths::*, references::*};
use mirai_annotations::{debug_checked_postcondition, debug_checked_precondition};
use std::collections::{BTreeMap, BTreeSet};

//**************************************************************************************************
// Definitions
//**************************************************************************************************

#[derive(Debug)]
pub struct Conflicts<Loc, Lbl: Ord> {
    /// These refs share a path
    pub equal: BTreeSet<RefID>,
    /// These refs extend the source ref by an unknown offset/lbl
    pub existential: BTreeMap<RefID, Loc>,
    /// These refs extend the source ref by a specified offset/lbl
    pub labeled: BTreeMap<Lbl, BTreeMap<RefID, Loc>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorrowSet<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> {
    map: BTreeMap<RefID, Ref<Loc, Instr, Lbl>>,
    next_id: usize,
}

impl<Loc, Lbl: Ord> Conflicts<Loc, Lbl> {
    pub fn is_empty(&self) -> bool {
        let Conflicts {
            equal,
            existential,
            labeled,
        } = self;
        equal.is_empty() && existential.is_empty() && labeled.is_empty()
    }
}

impl<Loc: Copy, Instr: Copy + Ord + std::fmt::Display, Lbl: Clone + Ord + std::fmt::Display>
    BorrowSet<Loc, Instr, Lbl>
{
    pub fn new<K: Ord>(
        pinned_initial_refs: impl IntoIterator<Item = (K, bool, Option<(Loc, Lbl)>)>,
    ) -> (Self, BTreeMap<K, RefID>) {
        let mut s = Self {
            map: BTreeMap::new(),
            next_id: 0,
        };
        let ref_ids = pinned_initial_refs
            .into_iter()
            .map(|(k, mutable, initial_lbl_opt)| {
                (k, s.add_ref(Ref::pinned(mutable, initial_lbl_opt)))
            })
            .collect();
        debug_checked_postcondition!((0..s.next_id).all(|i| s.map.contains_key(&RefID(i))));
        debug_checked_postcondition!(s.map.values().all(|ref_| ref_.is_pinned()));
        (s, ref_ids)
    }

    fn add_ref(&mut self, ref_: Ref<Loc, Instr, Lbl>) -> RefID {
        let id = RefID(self.next_id);
        self.next_id += 1;
        let old_value = self.map.insert(id, ref_);
        assert!(old_value.is_none());
        id
    }

    fn extend(
        &mut self,
        sources: impl IntoIterator<Item = RefID>,
        loc: Loc,
        mutable: bool,
        extension: Offset<Instr, Lbl>,
    ) -> RefID {
        let mut paths = BTreeSet::new();
        for source in sources {
            for path in self.map[&source].paths() {
                paths.insert(path.extend(loc.clone(), extension.clone()));
            }
        }
        if paths.is_empty() {
            paths.insert(BorrowPath::initial(loc, extension));
        }
        self.add_ref(Ref::new(mutable, paths))
    }

    //**********************************************************************************************
    // Ref API
    //**********************************************************************************************

    /// checks if the given reference is mutable or not
    pub fn is_mutable(&self, id: RefID) -> bool {
        self.map[&id].is_mutable()
    }

    pub fn make_copy(&mut self, loc: Loc, id: RefID, mutable: bool) -> RefID {
        let ref_ = self.map[&id].make_copy(loc, mutable);
        self.add_ref(ref_)
    }

    pub fn extend_by_label(
        &mut self,
        sources: impl IntoIterator<Item = RefID>,
        loc: Loc,
        mutable: bool,
        lbl: Lbl,
    ) -> RefID {
        self.extend(sources, loc, mutable, Offset::Labeled(lbl))
    }

    pub fn extend_by_unknown(
        &mut self,
        sources: impl IntoIterator<Item = RefID>,
        loc: Loc,
        mutable: bool,
        instr: Instr,
        ref_created_at_instr: usize,
    ) -> RefID {
        self.extend(
            sources,
            loc,
            mutable,
            Offset::Existential((instr, ref_created_at_instr)),
        )
    }

    pub fn move_into_pinned(&mut self, loc: Loc, pinned: RefID, other: RefID) {
        if pinned == other {
            return;
        }
        assert!(self.map[&pinned].is_pinned());
        let new_paths = self.map[&other].copy_paths(loc);
        if !self.is_pinned_released(pinned) {
            self.release(pinned);
        }
        if !self.map[&other].is_pinned() || !self.is_pinned_released(other) {
            self.release(other);
        }
        self.map.get_mut(&pinned).unwrap().add_paths(new_paths);
    }

    pub fn release(&mut self, id: RefID) {
        let ref_ = self.map.get_mut(&id).unwrap();
        if ref_.is_pinned() {
            assert!(!ref_.paths().is_empty());
            ref_.release_paths()
        } else {
            self.map.remove(&id).unwrap();
        }
    }

    //**********************************************************************************************
    // Query API
    //**********************************************************************************************

    pub fn borrowed_by(&self, id: RefID, only_mutable: bool) -> Conflicts<Loc, Lbl> {
        let mut equal = BTreeSet::new();
        let mut existential = BTreeMap::new();
        let mut labeled = BTreeMap::new();
        for path in self.map[&id].paths() {
            for (other_id, other_ref) in &self.map {
                let other_id = *other_id;
                if id == other_id {
                    continue;
                }
                if only_mutable && !self.is_mutable(other_id) {
                    continue;
                }

                for other_path in other_ref.paths() {
                    match path.compare(other_path) {
                        Ordering::Incomparable | Ordering::LeftExtendsRight => (),
                        Ordering::Equal => {
                            equal.insert(other_id);
                        }
                        Ordering::RightExtendsLeft(Offset::Existential(_)) => {
                            existential.insert(other_id, other_path.loc.clone());
                        }
                        Ordering::RightExtendsLeft(Offset::Labeled(lbl)) => {
                            labeled
                                .entry(lbl.clone())
                                .or_insert_with(BTreeMap::new)
                                .insert(other_id, other_path.loc.clone());
                        }
                    }
                }
            }
        }

        debug_checked_postcondition!(labeled.values().all(|refs| !refs.is_empty()));
        Conflicts {
            equal,
            existential,
            labeled,
        }
    }

    pub fn all_starting_with_label(&self, lbl: &Lbl) -> BTreeMap<RefID, Loc> {
        self.all_starting_with_predicate(|l| l == lbl)
    }

    pub fn all_starting_with_predicate(
        &self,
        mut p: impl FnMut(&Lbl) -> bool,
    ) -> BTreeMap<RefID, Loc> {
        let mut map = BTreeMap::new();
        for (id, ref_) in &self.map {
            for path in ref_.paths() {
                match path.path.first() {
                    Offset::Labeled(lbl) if p(lbl) => {
                        map.insert(*id, path.loc);
                    }
                    _ => (),
                }
            }
        }
        map
    }

    /// Returns true iff a pinned id has no borrows
    pub fn is_pinned_released(&self, id: RefID) -> bool {
        let ref_ = &self.map[&id];
        assert!(ref_.is_pinned());
        ref_.paths().is_empty()
    }

    //**********************************************************************************************
    // Joining
    //**********************************************************************************************

    pub fn is_covered_by(&self, other: &Self) -> bool {
        self.unmatched_paths(other).is_empty()
    }

    fn unmatched_paths<'a>(
        &self,
        other: &'a Self,
    ) -> BTreeMap<RefID, BTreeSet<&'a BorrowPath<Loc, Instr, Lbl>>> {
        let mut unmatched = BTreeMap::new();
        for (id, other_ref) in &other.map {
            let self_ref = &self.map[id];
            let self_paths = self_ref.paths();
            for other_path in other_ref.paths() {
                // Otherwise, check if there is any path in self s.t. the other path is an extension
                // of it
                // In other words, does there exist a path in self that covers the other path
                if self_paths.iter().any(|self_path| {
                    matches!(
                        self_path.compare(other_path),
                        Ordering::Equal | Ordering::RightExtendsLeft(_)
                    )
                }) {
                    continue;
                }

                unmatched
                    .entry(*id)
                    .or_insert_with(BTreeSet::new)
                    .insert(other_path);
            }
        }
        unmatched
    }

    pub fn join(&self, other: &Self) -> Self {
        debug_checked_precondition!(self.map.keys().all(|id| other.map.contains_key(id)));
        debug_checked_precondition!(other.map.keys().all(|id| self.map.contains_key(id)));
        debug_checked_precondition!(self.map.keys().all(|id| self.map[&id].is_pinned()));
        debug_checked_precondition!(other.map.keys().all(|id| self.map[&id].is_pinned()));

        let mut joined = self.clone();
        joined.next_id = joined.map.len();
        assert!(joined.map.keys().all(|id| id.0 < joined.next_id));
        for (id, unmatched_borrowed_by) in self.unmatched_paths(other) {
            joined
                .map
                .get_mut(&id)
                .unwrap()
                .add_paths(unmatched_borrowed_by.into_iter().cloned())
        }
        joined
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
        for (id, ref_) in &self.map {
            let mut_ = if ref_.is_mutable() { "mut " } else { "imm " };
            let pinned = if ref_.is_pinned() { "#pinned" } else { "" };
            println!("{}{}{}: {{", mut_, id.0, pinned);
            for path in ref_.paths() {
                println!("    {},", path.path.to_string());
            }
            println!("}},")
        }
    }
}
