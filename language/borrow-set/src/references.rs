// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::paths::*;
use std::{
    cmp,
    collections::BTreeSet,
    fmt::{self, Debug},
    iter,
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
    /// true if mutable, false otherwise
    pub(crate) mutable: bool,
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

impl RefID {
    /// Creates a new reference id from the given number
    pub const fn new(x: usize) -> Self {
        RefID(x)
    }

    /// Returns the number representing this reference id.
    pub fn number(&self) -> usize {
        self.0
    }
}

impl<Loc: Copy, Instr: Copy + Ord, Lbl: Clone + Ord> Ref<Loc, Instr, Lbl> {
    /// Create a new root reference
    pub(crate) fn new(
        mutable: bool,
        loc: Loc,
        mut paths: BTreeSet<BorrowPath<Loc, Instr, Lbl>>,
    ) -> Self {
        if paths.is_empty() {
            paths.insert(BorrowPath {
                loc,
                path: Path::empty(),
            });
        }
        Self { mutable, paths }
    }

    pub(crate) fn paths(&self) -> &BTreeSet<BorrowPath<Loc, Instr, Lbl>> {
        assert!(!self.paths.is_empty());
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

    pub(crate) fn extended_by<'a>(&self, other: &'a Self) -> Ordering<'a, Instr, Lbl>
    where
        Instr: Eq,
        Lbl: Eq,
    {
        self.path.extended_by(&other.path)
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
