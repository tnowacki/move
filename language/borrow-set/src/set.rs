// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{paths::*, references::*};
use mirai_annotations::{debug_checked_postcondition, debug_checked_precondition};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
};

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

pub struct Parents<Loc, Lbl: Ord> {
    /// Not quite parents, but exactly equal
    pub equal: BTreeSet<RefID>,
    /// The ref in question extends these refs at an existential
    pub existential: BTreeMap<RefID, Loc>,
    /// The ref in question extends these refs at this label
    pub labeled: BTreeMap<Lbl, BTreeMap<RefID, Loc>>,
}

#[derive(Clone)]
pub struct QueryFilter<'a> {
    /// only query over these refs
    mask: Option<&'a BTreeSet<RefID>>,
    /// only query over these mutable statuses
    mutable: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorrowSet<Loc: Copy, Lbl: Clone + Ord> {
    map: BTreeMap<RefID, Ref<Loc, Lbl>>,
    next_id: usize,
}

//**************************************************************************************************
// impls
//**************************************************************************************************

// pub(crate) fn extend_btree_map<Loc>(m1: &mut BTreeMap<RefID, Loc>, m2: BTreeMap<RefID, Loc>) {
//     for (id, other_loc) in m2 {
//         if !m1.contains_key(&id) {
//             m1.insert(id, other_loc);
//         }
//     }
// }

// pub(crate) fn merge_btree_map<Loc>(
//     mut m1: BTreeMap<RefID, Loc>,
//     m2: BTreeMap<RefID, Loc>,
// ) -> BTreeMap<RefID, Loc> {
//     extend_btree_map(&mut m1, m2);
//     m1
// }

impl<Loc, Lbl: Ord> Conflicts<Loc, Lbl> {
    pub fn is_empty(&self) -> bool {
        let Conflicts {
            equal,
            existential,
            labeled,
        } = self;
        equal.is_empty() && existential.is_empty() && labeled.is_empty()
    }

    // pub(crate) fn extend(&mut self, other: Self) {
    //     let Conflicts {
    //         equal: other_equal,
    //         existential: other_existential,
    //         labeled: other_labeled,
    //     } = other;
    //     self.equal.extend(other_equal);
    //     extend_btree_map(&mut self.existential, other_existential);
    //     for (lbl, other_refs) in other_labeled {
    //         let self_refs = self.labeled.entry(lbl).or_insert_with(BTreeMap::new);
    //         extend_btree_map(self_refs, other_refs)
    //     }
    // }

    // pub(crate) fn merge(mut self, other: Self) -> Self {
    //     self.extend(other);
    //     self
    // }
}

impl<Loc, Lbl: Ord> Parents<Loc, Lbl> {
    pub fn is_empty(&self) -> bool {
        let Parents {
            equal,
            existential,
            labeled,
        } = self;
        equal.is_empty() && existential.is_empty() && labeled.is_empty()
    }

    // pub(crate) fn extend(&mut self, other: Self) {
    //     let Parents {
    //         equal: other_equal,
    //         existential: other_existential,
    //         labeled: other_labeled,
    //     } = other;
    //     self.equal.extend(other_equal);
    //     extend_btree_map(&mut self.existential, other_existential);
    //     for (lbl, other_refs) in other_labeled {
    //         let self_refs = self.labeled.get_mut(&lbl).unwrap();
    //         extend_btree_map(self_refs, other_refs)
    //     }
    // }

    // pub(crate) fn merge(mut self, other: Self) -> Self {
    //     self.extend(other);
    //     self
    // }
}

impl<'a> QueryFilter<'a> {
    pub fn empty() -> Self {
        QueryFilter {
            mask: None,
            mutable: None,
        }
    }

    pub fn is_mutable(mut self, mutable: bool) -> Self {
        self.mutable = Some(mutable);
        self
    }

    pub fn candidates(mut self, candidates: &'a BTreeSet<RefID>) -> Self {
        self.mask = Some(candidates);
        self
    }
}

macro_rules! filtered_iter {
    ($set:expr, $filter:expr) => {{
        let QueryFilter { mask, mutable } = $filter;
        $set.map
            .iter()
            .filter(move |(id, ref_)| {
                let satisfies_mutable = mutable
                    .map(|mutable_filter| ref_.is_mutable() == mutable_filter)
                    .unwrap_or(true);
                let satisfies_mask = mask.as_ref().map(|mask| mask.contains(id)).unwrap_or(true);
                satisfies_mutable && satisfies_mask
            })
            .map(|(id, ref_)| (*id, ref_))
    }};
}

impl<Loc: Copy, Lbl: Clone + Ord + std::fmt::Display> BorrowSet<Loc, Lbl> {
    pub fn new<K: Ord>(
        initial_refs: impl IntoIterator<Item = (K, bool, Loc, Lbl)>,
    ) -> (Self, BTreeMap<K, RefID>) {
        let mut s = Self {
            map: BTreeMap::new(),
            next_id: 0,
        };
        let ref_ids = initial_refs
            .into_iter()
            .map(|(k, mutable, loc, lbl)| {
                let mut paths = BTreeSet::new();
                paths.insert(BorrowPath::new(loc, None, vec![lbl], false));
                (k, s.add_ref(Ref::new(mutable, paths)))
            })
            .collect();
        debug_checked_postcondition!((0..s.next_id).all(|i| s.map.contains_key(&RefID(i))));
        (s, ref_ids)
    }

    fn add_ref(&mut self, ref_: Ref<Loc, Lbl>) -> RefID {
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
        offsets: Vec<Lbl>,
        ends_in_star: bool,
    ) -> RefID {
        // either the path ends in star or it has offsets, not both
        debug_assert!(ends_in_star ^ !offsets.is_empty());
        let mut paths = BTreeSet::new();
        for source in sources {
            // for each source/parent reference, extend that reference
            paths.insert(BorrowPath::new(
                loc,
                Some(source),
                offsets.clone(),
                ends_in_star,
            ));
        }
        if paths.is_empty() {
            // if there were no source/parent references, the extension comes from a root
            // i.e. a local or a global
            paths.insert(BorrowPath::new(loc, None, offsets, ends_in_star));
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

    pub fn make_copy(&mut self, id: RefID) -> RefID {
        self.map.get_mut(&id).unwrap().increment_count();
        id
    }

    pub fn freeze(&mut self, id: RefID) {
        self.map.get_mut(&id).unwrap().freeze()
    }

    pub fn extend_by_labels(
        &mut self,
        sources: impl IntoIterator<Item = RefID>,
        loc: Loc,
        mutable: bool,
        offsets: Vec<Lbl>,
    ) -> RefID {
        self.extend(sources, loc, mutable, offsets, false)
    }

    pub fn extend_by_unknown(
        &mut self,
        sources: impl IntoIterator<Item = RefID>,
        loc: Loc,
        mutable: bool,
    ) -> RefID {
        self.extend(sources, loc, mutable, vec![], true)
    }

    pub fn release(&mut self, id: RefID) {
        let ref_ = self.map.get_mut(&id).unwrap();
        if ref_.count() > 1 {
            ref_.decrement_count();
            return;
        }

        let ref_ = self.map.remove(&id).unwrap();
        let updated_paths = ref_
            .into_paths()
            .into_iter()
            .map(|mut bp| {
                bp.path.ends_in_star = true;
                bp
            })
            .collect();
        for other_ref in self.map.values_mut() {
            other_ref.release_parent(id, &updated_paths)
        }
    }

    //**********************************************************************************************
    // Query API
    //**********************************************************************************************

    pub fn borrowed_by(&self, id: RefID, filter: QueryFilter) -> Conflicts<Loc, Lbl> {
        let mut equal = BTreeSet::new();
        let mut existential = BTreeMap::new();
        let mut labeled = BTreeMap::new();
        for path in self.map[&id].paths() {
            let filtered = filtered_iter!(self, filter).filter(|(other_id, _)| &id != other_id);
            for (other_id, other_ref) in filtered {
                for other_path in other_ref.paths() {
                    match path.compare(other_path) {
                        Ordering::Incomparable | Ordering::LeftExtendsRight => (),
                        Ordering::Equal => {
                            equal.insert(other_id);
                        }
                        Ordering::RightExtendsLeft(Extension::Star) => {
                            existential.insert(other_id, other_path.loc.clone());
                        }
                        Ordering::RightExtendsLeft(Extension::Label(lbl)) => {
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

    pub fn borrows_from(&self, id: RefID, filter: QueryFilter) -> Parents<Loc, Lbl> {
        let mut equal = BTreeSet::new();
        let mut existential = BTreeMap::new();
        let mut labeled = BTreeMap::new();
        for path in self.map[&id].paths() {
            let filtered = filtered_iter!(self, filter).filter(|(other_id, _)| &id != other_id);
            for (other_id, other_ref) in filtered {
                for other_path in other_ref.paths() {
                    match other_path.compare(path) {
                        Ordering::Incomparable | Ordering::LeftExtendsRight => (),
                        Ordering::Equal => {
                            equal.insert(other_id);
                        }
                        Ordering::RightExtendsLeft(Extension::Star) => {
                            existential.insert(other_id, path.loc);
                        }
                        Ordering::RightExtendsLeft(Extension::Label(lbl)) => {
                            labeled
                                .entry(lbl.clone())
                                .or_insert_with(BTreeMap::new)
                                .insert(other_id, path.loc);
                        }
                    }
                }
            }
        }

        debug_checked_postcondition!(labeled.values().all(|refs| !refs.is_empty()));
        Parents {
            equal,
            existential,
            labeled,
        }
    }

    // pub fn all_starting_with_label(&self, lbl: &Lbl) -> BTreeMap<RefID, Loc> {
    //     self.all_starting_with_predicate(|(_, l)| l == lbl)
    // }

    pub fn all_start_with_ref(&self, id: RefID) -> BTreeMap<RefID, Loc> {
        self.all_starting_with_predicate(|start_opt, _| {
            start_opt.map(|start| id == start).unwrap_or(false)
        })
    }

    pub fn all_starting_with_predicate(
        &self,
        mut p: impl FnMut(Option<RefID>, Option<&Lbl>) -> bool,
    ) -> BTreeMap<RefID, Loc> {
        let mut map = BTreeMap::new();
        for (id, ref_) in &self.map {
            for path in ref_.paths() {
                let ref_start = path.path.ref_start;
                let lbl_opt = path.path.first();
                if p(ref_start, lbl_opt) {
                    map.insert(*id, path.loc);
                }
            }
        }
        map
    }

    //**********************************************************************************************
    // Joining
    //**********************************************************************************************

    pub fn covers(&self, other: &Self) -> bool {
        self.consistent_world(other);
        for (id, other_ref) in &other.map {
            let self_ref = &self.map[id];
            let self_paths = self_ref.paths();
            for other_path in other_ref.paths() {
                // Otherwise, check if there is any path in self s.t. the other path is an extension
                // of it
                // In other words, does there exist a path in self that covers the other path
                let other_path_is_covered = self_paths.iter().any(|self_path| {
                    matches!(
                        self_path.compare(other_path),
                        Ordering::Equal | Ordering::RightExtendsLeft(_)
                    )
                });
                if !other_path_is_covered {
                    return false;
                }
            }
        }
        return true;
    }

    pub fn join(&self, other: &Self) -> Self {
        self.consistent_world(other);
        let mut joined = self.clone();
        for (id, other_ref) in &other.map {
            let self_ref = &self.map[id];
            let self_paths = self_ref.paths();
            for other_path in other_ref.paths() {
                // Otherwise, check if there is any path in self s.t. the other path is an extension
                // of it
                // In other words, does there exist a path in self that covers the other path
                let other_path_is_covered = self_paths.iter().any(|self_path| {
                    matches!(
                        self_path.compare(other_path),
                        Ordering::Equal | Ordering::RightExtendsLeft(_)
                    )
                });
                if !other_path_is_covered {
                    joined.map.get_mut(id).unwrap().add_path(other_path.clone())
                }
            }
        }
        self.consistent_world(&joined);
        joined.set_next_id_after_join();
        joined
    }

    pub(crate) fn set_next_id_after_join(&mut self) {
        let n = self.map.len();
        debug_checked_precondition!(self.map.keys().all(|id| id.0 < n));
        self.next_id = n;
    }

    //**********************************************************************************************
    // Invariants
    //**********************************************************************************************

    pub(crate) fn consistent_world(&self, other: &Self) {
        // same keys
        debug_checked_postcondition!(self.map.keys().all(|id| other.map.contains_key(id)));
        debug_checked_postcondition!(other.map.keys().all(|id| self.map.contains_key(id)));
        // next_id is the same
        debug_checked_postcondition!(self.next_id == other.next_id);
        // mut status same
        // count same
        if cfg!(debug_assertions) {
            for id in self.map.keys() {
                debug_checked_postcondition!(
                    self.map[id].is_mutable() == other.map[id].is_mutable()
                );
                debug_checked_postcondition!(self.map[id].count() == other.map[id].count());
            }
        }
    }

    //**********************************************************************************************
    // Util
    //**********************************************************************************************

    #[allow(dead_code)]
    pub fn display(&self)
    where
        Lbl: std::fmt::Display,
    {
        for (id, ref_) in &self.map {
            let mut_ = if ref_.is_mutable() { "mut" } else { "imm" };
            println!("    {} {}: {{", mut_, id.0);
            for path in ref_.paths() {
                println!("        {},", path.path.to_string());
            }
            println!("    }},")
        }
    }
}

// // Copyright (c) The Diem Core Contributors
// // SPDX-License-Identifier: Apache-2.0

// use crate::{
//     map::{merge_btree_map, BorrowMap},
//     references::*,
// };
// use mirai_annotations::{
//     debug_checked_postcondition, debug_checked_precondition, debug_checked_verify,
// };
// use std::collections::BTreeMap;

// //**************************************************************************************************
// // Definitions
// //**************************************************************************************************

// pub use crate::map::{Conflicts, Parents, QueryFilter};

// #[derive(Clone, Debug, Default)]
// pub struct BorrowSet<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> {
//     worlds: Vec<BorrowMap<Loc, Instr, Lbl>>,
// }

// //**************************************************************************************************
// // impls
// //**************************************************************************************************

// impl<Loc: Copy, Instr: Copy + Ord + std::fmt::Display, Lbl: Clone + Ord + std::fmt::Display>
//     BorrowSet<Loc, Instr, Lbl>
// {
//     pub fn new<K: Ord>(
//         pinned_initial_refs: impl IntoIterator<Item = (K, bool, Option<(Loc, Lbl)>)>,
//     ) -> (Self, BTreeMap<K, RefID>) {
//         let (map, ids) = BorrowMap::new(pinned_initial_refs);
//         (Self { worlds: vec![map] }, ids)
//     }

//     //**********************************************************************************************
//     // Ref API
//     //**********************************************************************************************

//     /// checks if the given reference is mutable or not
//     pub fn is_mutable(&self, id: RefID) -> bool {
//         debug_checked_precondition!(self.satisfies_invariant());
//         self.worlds[0].is_mutable(id)
//     }

//     pub fn make_copy(&mut self, loc: Loc, id: RefID, mutable: bool) -> RefID {
//         debug_checked_precondition!(self.satisfies_invariant());
//         let mut worlds = self.worlds.iter_mut();
//         let new_id = worlds.next().unwrap().make_copy(loc, id, mutable);
//         for world in worlds {
//             let other_id = world.make_copy(loc, id, mutable);
//             debug_checked_verify!(new_id == other_id)
//         }
//         new_id
//     }

//     pub fn extend_by_label(
//         &mut self,
//         sources: impl IntoIterator<Item = RefID>,
//         loc: Loc,
//         mutable: bool,
//         lbl: Lbl,
//     ) -> RefID {
//         debug_checked_precondition!(self.satisfies_invariant());
//         let sources = sources.into_iter().collect::<Vec<_>>();
//         let mut worlds = self.worlds.iter_mut();
//         let new_id =
//             worlds
//                 .next()
//                 .unwrap()
//                 .extend_by_label(sources.clone(), loc, mutable, lbl.clone());
//         for world in worlds {
//             let other_id = world.extend_by_label(sources.clone(), loc, mutable, lbl.clone());
//             debug_checked_verify!(new_id == other_id)
//         }
//         new_id
//     }

//     pub fn extend_by_unknown(
//         &mut self,
//         sources: impl IntoIterator<Item = RefID>,
//         loc: Loc,
//         mutable: bool,
//         instr: Instr,
//         ref_created_at_instr: usize,
//     ) -> RefID {
//         debug_checked_precondition!(self.satisfies_invariant());
//         let sources = sources.into_iter().collect::<Vec<_>>();
//         let mut worlds = self.worlds.iter_mut();
//         let new_id = worlds.next().unwrap().extend_by_unknown(
//             sources.clone(),
//             loc,
//             mutable,
//             instr,
//             ref_created_at_instr,
//         );
//         for world in worlds {
//             let other_id =
//                 world.extend_by_unknown(sources.clone(), loc, mutable, instr, ref_created_at_instr);
//             debug_checked_verify!(new_id == other_id)
//         }
//         new_id
//     }

//     pub fn move_into_pinned(&mut self, loc: Loc, pinned: RefID, other: RefID) {
//         debug_checked_precondition!(self.satisfies_invariant());
//         for world in &mut self.worlds {
//             world.move_into_pinned(loc, pinned, other)
//         }
//     }

//     pub fn release(&mut self, id: RefID) {
//         debug_checked_precondition!(self.satisfies_invariant());
//         for world in &mut self.worlds {
//             world.release(id)
//         }
//     }

//     //**********************************************************************************************
//     // Query API
//     //**********************************************************************************************

//     pub fn borrowed_by(&self, id: RefID, filter: QueryFilter) -> Conflicts<Loc, Lbl> {
//         debug_checked_precondition!(self.satisfies_invariant());
//         self.worlds
//             .iter()
//             .map(|world| world.borrowed_by(id, filter.clone()))
//             .reduce(|conflicts1, conflicts2| conflicts1.merge(conflicts2))
//             .unwrap()
//     }

//     pub fn borrows_from(&self, id: RefID, filter: QueryFilter) -> Parents<Loc, Lbl> {
//         debug_checked_precondition!(self.satisfies_invariant());
//         self.worlds
//             .iter()
//             .map(|world| world.borrows_from(id, filter.clone()))
//             .reduce(|parents1, parents2| parents1.merge(parents2))
//             .unwrap()
//     }

//     pub fn all_starting_with_label(&self, lbl: &Lbl) -> BTreeMap<RefID, Loc> {
//         debug_checked_precondition!(self.satisfies_invariant());
//         self.worlds
//             .iter()
//             .map(|world| world.all_starting_with_label(lbl))
//             .reduce(|refs1, refs2| merge_btree_map(refs1, refs2))
//             .unwrap()
//     }

//     pub fn all_starting_with_predicate(
//         &self,
//         mut p: impl FnMut(&Lbl) -> bool,
//     ) -> BTreeMap<RefID, Loc> {
//         debug_checked_precondition!(self.satisfies_invariant());
//         self.worlds
//             .iter()
//             .map(|world| world.all_starting_with_predicate(&mut p))
//             .reduce(|refs1, refs2| merge_btree_map(refs1, refs2))
//             .unwrap()
//     }

//     /// Returns true iff a pinned id has no borrows
//     pub fn is_pinned_released(&self, id: RefID) -> bool {
//         debug_checked_precondition!(self.satisfies_invariant());
//         self.worlds[0].is_pinned_released(id)
//     }

//     //**********************************************************************************************
//     // Joining
//     //**********************************************************************************************

//     pub fn covers(&self, other: &Self) -> bool {
//         other.worlds.iter().all(|other_world| {
//             self.worlds
//                 .iter()
//                 .any(|self_world| self_world.covers(other_world))
//         })
//     }

//     pub fn join(&self, other: &Self) -> Self {
//         debug_checked_precondition!(self.satisfies_invariant());
//         debug_checked_precondition!(other.satisfies_invariant());
//         let mut joined = self.clone();
//         let not_covered_worlds = other
//             .worlds
//             .iter()
//             .filter(|other_world| {
//                 !self
//                     .worlds
//                     .iter()
//                     .any(|self_world| self_world.covers(other_world))
//             })
//             .cloned();
//         joined.worlds.extend(not_covered_worlds);
//         for world in &mut joined.worlds {
//             world.set_next_id_after_join()
//         }
//         debug_checked_postcondition!(joined.satisfies_invariant());
//         joined
//     }

//     //**********************************************************************************************
//     // Invariants
//     //**********************************************************************************************

//     pub fn satisfies_invariant(&self) -> bool {
//         if self.worlds.is_empty() {
//             return false;
//         }
//         let world_0 = &self.worlds[0];
//         let mut other_worlds = self.worlds.iter().skip(1);
//         other_worlds.all(|other| world_0.consistent_world(other))
//     }

//     //**********************************************************************************************
//     // Util
//     //**********************************************************************************************

//     #[allow(dead_code)]
//     pub fn display(&self)
//     where
//         Instr: std::fmt::Display,
//         Lbl: std::fmt::Display,
//     {
//         for world in &self.worlds {
//             println!("{{");
//             world.display();
//             println!("}},");
//         }
//     }
// }
