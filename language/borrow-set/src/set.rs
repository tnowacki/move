// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{paths::*, references::*};
use mirai_annotations::{debug_checked_postcondition, debug_checked_precondition};
use std::collections::{BTreeMap, BTreeSet};

//**************************************************************************************************
// Definitions
//**************************************************************************************************

pub struct BorrowedBy<Loc, Lbl: Ord> {
    /// These refs share a path
    pub equal: BTreeSet<RefID>,
    /// These refs extend the source ref by an unknown offset/lbl
    pub existential: BTreeMap<RefID, Loc>,
    /// These refs extend the source ref by a specified offset/lbl
    pub labeled: BTreeMap<Lbl, BTreeMap<RefID, Loc>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorrowSet<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord>(
    BTreeMap<RefID, Ref<Loc, Instr, Lbl>>,
);

impl<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> BorrowSet<Loc, Instr, Lbl> {
    /// creates an empty borrow graph
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    fn new_ref(
        &mut self,
        mutable: bool,
        loc: Loc,
        paths: BTreeSet<BorrowPath<Loc, Instr, Lbl>>,
    ) -> RefID {
        let id = RefID::new(self.0.len());
        let ref_data = Ref::new(mutable, loc, paths);
        self.0.insert(id, ref_data).unwrap();
        id
    }

    pub fn extend(
        &mut self,
        sources: impl IntoIterator<Item = RefID>,
        loc: Loc,
        mutable: bool,
        extension: Offset<Instr, Lbl>,
    ) -> RefID {
        let mut paths = BTreeSet::new();
        for source in sources {
            for path in self.0[&source].paths() {
                paths.insert(path.extend(loc.clone(), extension.clone()));
            }
        }
        self.new_ref(mutable, loc, paths)
    }

    //**********************************************************************************************
    // Ref API
    //**********************************************************************************************

    /// checks if the given reference is mutable or not
    pub fn is_mutable(&self, id: RefID) -> bool {
        self.0[&id].mutable
    }

    pub fn copy_ref(&mut self, id: RefID) -> RefID {
        let ref_ = self.0[&id].clone();
        let id = RefID::new(self.0.len());
        self.0.insert(id, ref_).unwrap();
        id
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

    pub fn release(&mut self, id: RefID) {
        self.0.remove(&id).unwrap();
    }

    pub fn borrowed_by(&self, id: RefID) -> BorrowedBy<Loc, Lbl> {
        let mut equal = BTreeSet::new();
        let mut existential = BTreeMap::new();
        let mut labeled = BTreeMap::new();
        for path in self.0[&id].paths() {
            for (other_id, other_ref) in &self.0 {
                if id == *other_id {
                    continue;
                }
                for other_path in other_ref.paths() {
                    match path.extended_by(other_path) {
                        Ordering::Other => (),
                        Ordering::Equal => {
                            equal.insert(*other_id);
                        }
                        Ordering::Extension(Offset::Existential(_)) => {
                            existential.insert(*other_id, other_path.loc.clone());
                        }
                        Ordering::Extension(Offset::Labeled(lbl)) => {
                            labeled
                                .entry(lbl.clone())
                                .or_insert_with(BTreeMap::new)
                                .insert(*other_id, other_path.loc.clone());
                        }
                    }
                }
            }
        }
        BorrowedBy {
            equal,
            existential,
            labeled,
        }
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
        for (id, other_ref) in &other.0 {
            let self_ref = &self.0[id];
            let self_paths = self_ref.paths();
            for other_path in other_ref.paths() {
                // optimization for exact path
                if self_paths.contains(other_path) {
                    continue;
                }
                // Otherwise, check if there is any path in self s.t. the other path is an extension
                // of it
                // In other words, does there exist a path in self that covers the other path
                if self_paths.iter().any(|self_path| {
                    matches!(self_path.extended_by(other_path), Ordering::Extension(_))
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
        debug_checked_precondition!(self.0.keys().all(|id| other.0.contains_key(id)));
        debug_checked_precondition!(other.0.keys().all(|id| self.0.contains_key(id)));

        let mut joined = self.clone();
        for (id, unmatched_borrowed_by) in self.unmatched_paths(other) {
            joined
                .0
                .get_mut(&id)
                .unwrap()
                .add_paths(unmatched_borrowed_by.into_iter().cloned())
        }
        joined
    }
}
