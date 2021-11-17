// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::paths::*;
use std::{
    cmp,
    collections::BTreeSet,
    fmt::{self, Debug},
    iter,
    num::NonZeroUsize,
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
pub(crate) struct Ref<Loc: Copy, Lbl: Clone + Ord> {
    /// true if mutable, false otherwise
    mutable: bool,
    /// Set of paths defining possible locations for this reference
    paths: BTreeSet<BorrowPath<Loc, Lbl>>,
    /// The number of references with this id
    pub(crate) count: usize,
}

#[derive(Clone)]
/// A path + a location where it was added
pub(crate) struct BorrowPath<Loc, Lbl> {
    /// The location
    pub(crate) loc: Loc,
    /// The actual path data
    pub(crate) path: Path<Lbl>,
}

//**************************************************************************************************
// Impls
//**************************************************************************************************

impl<Loc: Copy, Lbl: Clone + Ord> Ref<Loc, Lbl> {
    pub(crate) fn new(mutable: bool, paths: BTreeSet<BorrowPath<Loc, Lbl>>) -> Self {
        Self {
            mutable,
            paths,
            count: 1,
        }
    }

    pub(crate) fn is_mutable(&self) -> bool {
        self.mutable
    }

    pub(crate) fn paths(&self) -> &BTreeSet<BorrowPath<Loc, Lbl>> {
        &self.paths
    }

    pub(crate) fn into_paths(self) -> BTreeSet<BorrowPath<Loc, Lbl>> {
        self.paths
    }

    pub(crate) fn add_path(&mut self, additional: BorrowPath<Loc, Lbl>) {
        let is_additional = self.paths.insert(additional);
        assert!(is_additional);
    }

    pub(crate) fn add_paths(&mut self, additional: impl IntoIterator<Item = BorrowPath<Loc, Lbl>>) {
        self.paths.extend(additional)
    }

    pub(crate) fn release_parent(
        &mut self,
        parent_ref: RefID,
        updated_paths: &BTreeSet<BorrowPath<Loc, Lbl>>,
    ) {
        debug_assert!(updated_paths.iter().all(|bp| bp.path.ends_in_star));
        let before_size = self.paths.len();
        self.paths = std::mem::take(&mut self.paths)
            .into_iter()
            .filter(|bp| bp.path.ends_in_star)
            .collect();
        let after_size = self.paths.len();
        debug_assert!(before_size >= after_size);
        if before_size > after_size {
            self.paths.extend(updated_paths.clone());
        }
    }

    pub(crate) fn freeze(&mut self) {
        self.mutable = false
    }

    pub(crate) fn increment_count(&mut self) {
        self.count += 1
    }

    pub(crate) fn decrement_count(&mut self) {
        assert!(self.count > 1);
        self.count -= 1
    }

    pub(crate) fn count(&self) -> usize {
        self.count
    }
}

impl<Loc, Lbl> BorrowPath<Loc, Lbl> {
    pub(crate) fn new(
        loc: Loc,
        ref_start: Option<RefID>,
        offsets: Vec<Lbl>,
        ends_in_star: bool,
    ) -> Self {
        Self {
            loc,
            path: Path::new(ref_start, offsets, ends_in_star),
        }
    }

    pub(crate) fn compare<'a>(&self, other: &'a Self) -> Ordering<'a, Lbl>
    where
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

impl<Loc: Copy, Lbl: Clone + Ord> PartialEq for BorrowPath<Loc, Lbl> {
    fn eq(&self, other: &BorrowPath<Loc, Lbl>) -> bool {
        self.path == other.path
    }
}

impl<Loc: Copy, Lbl: Clone + Ord> Eq for BorrowPath<Loc, Lbl> {}

impl<Loc: Copy, Lbl: Clone + Ord> PartialOrd for BorrowPath<Loc, Lbl> {
    fn partial_cmp(&self, other: &BorrowPath<Loc, Lbl>) -> Option<cmp::Ordering> {
        self.path.partial_cmp(&other.path)
    }
}

impl<Loc: Copy, Lbl: Clone + Ord> Ord for BorrowPath<Loc, Lbl> {
    fn cmp(&self, other: &BorrowPath<Loc, Lbl>) -> cmp::Ordering {
        self.path.cmp(&other.path)
    }
}

impl<Loc: Copy, Lbl: Clone + Ord + Debug> Debug for BorrowPath<Loc, Lbl> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.fmt(f)
    }
}
