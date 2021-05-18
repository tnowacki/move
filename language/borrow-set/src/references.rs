// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::paths::*;
use mirai_annotations::debug_checked_verify;
use std::{
    cmp,
    collections::BTreeSet,
    fmt::{self, Debug},
    iter,
    iter::FromIterator,
};

//**************************************************************************************************
// Definitions
//**************************************************************************************************

/// Unique identifier for the reference
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RefID(pub(crate) usize);

/// Represents the borrow relationships and information for a node in the borrow graph, i.e
/// for a single reference
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Ref<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> {
    pinned: bool,
    /// true if mutable, false otherwise
    mutable: bool,
    /// Set of paths defining possible locations for this reference
    paths: BTreeSet<BorrowPath<Loc, Instr, Lbl>>,
}

#[derive(Clone)]
/// A path + a location where it was added
pub(crate) struct BorrowPath<Loc, Instr, Lbl> {
    /// The location
    pub(crate) loc: Loc,
    /// The actual path data
    pub(crate) path: Path<Instr, Lbl>,
}

//**************************************************************************************************
// Impls
//**************************************************************************************************

impl<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> Ref<Loc, Instr, Lbl> {
    pub(crate) fn pinned(mutable: bool, init_offset: Option<(Loc, Lbl)>) -> Self {
        let paths = match init_offset {
            Some((loc, lbl)) => {
                BTreeSet::from_iter(vec![BorrowPath::initial(loc, Offset::Labeled(lbl))])
            }
            None => BTreeSet::new(),
        };
        Self {
            pinned: true,
            mutable,
            paths,
        }
    }

    pub(crate) fn new(mutable: bool, paths: BTreeSet<BorrowPath<Loc, Instr, Lbl>>) -> Self {
        Self {
            pinned: false,
            mutable,
            paths,
        }
    }

    pub(crate) fn make_copy(&self, loc: Loc, mutable: bool) -> Self {
        let paths = self.copy_paths(loc);
        Self {
            pinned: false,
            mutable,
            paths,
        }
    }

    pub(crate) fn copy_paths(&self, loc: Loc) -> BTreeSet<BorrowPath<Loc, Instr, Lbl>> {
        self.paths
            .iter()
            .map(|path| {
                let mut new_path = path.clone();
                new_path.loc = loc;
                new_path
            })
            .collect()
    }

    pub(crate) fn is_mutable(&self) -> bool {
        self.mutable
    }

    pub(crate) fn is_pinned(&self) -> bool {
        self.pinned
    }

    pub(crate) fn release_paths(&mut self) {
        assert!(self.pinned);
        self.paths = BTreeSet::new()
    }

    pub(crate) fn paths(&self) -> &BTreeSet<BorrowPath<Loc, Instr, Lbl>> {
        debug_checked_verify!(self.pinned || !self.paths.is_empty());
        &self.paths
    }

    pub(crate) fn add_paths(
        &mut self,
        additional: impl IntoIterator<Item = BorrowPath<Loc, Instr, Lbl>>,
    ) {
        self.paths.extend(additional)
    }
}

impl<Loc, Instr, Lbl> BorrowPath<Loc, Instr, Lbl> {
    pub(crate) fn initial(loc: Loc, offset: Offset<Instr, Lbl>) -> Self {
        Self {
            loc,
            path: Path::initial(offset),
        }
    }

    pub(crate) fn extend(&self, loc: Loc, extension: Offset<Instr, Lbl>) -> Self
    where
        Instr: Copy,
        Lbl: Clone,
    {
        Self {
            loc,
            path: self.path.extend(extension),
        }
    }

    pub(crate) fn compare<'a>(&self, other: &'a Self) -> Ordering<'a, Instr, Lbl>
    where
        Instr: Eq,
        Lbl: Eq,
    {
        self.path.compare(&other.path)
    }
}

//**************************************************************************************************
// traits
//**************************************************************************************************

impl IntoIterator for RefID {
    type Item = RefID;
    type IntoIter = iter::Once<RefID>;
    fn into_iter(self) -> Self::IntoIter {
        iter::once(self)
    }
}

impl<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> PartialEq for BorrowPath<Loc, Instr, Lbl> {
    fn eq(&self, other: &BorrowPath<Loc, Instr, Lbl>) -> bool {
        self.path == other.path
    }
}

impl<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> Eq for BorrowPath<Loc, Instr, Lbl> {}

impl<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> PartialOrd for BorrowPath<Loc, Instr, Lbl> {
    fn partial_cmp(&self, other: &BorrowPath<Loc, Instr, Lbl>) -> Option<cmp::Ordering> {
        self.path.partial_cmp(&other.path)
    }
}

impl<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> Ord for BorrowPath<Loc, Instr, Lbl> {
    fn cmp(&self, other: &BorrowPath<Loc, Instr, Lbl>) -> cmp::Ordering {
        self.path.cmp(&other.path)
    }
}

impl<Loc: Copy, Instr: Copy + Ord + Debug, Lbl: Clone + Ord + Debug> Debug
    for BorrowPath<Loc, Instr, Lbl>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.fmt(f)
    }
}
