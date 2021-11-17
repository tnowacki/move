// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0
use crate::references::RefID;
use mirai_annotations::debug_checked_precondition;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Extension<Lbl> {
    Label(Lbl),
    Star,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Path<Lbl> {
    pub(crate) ref_start: Option<RefID>,
    pub(crate) path: Vec<Lbl>,
    pub(crate) ends_in_star: bool,
}

#[derive(Debug)]
pub enum Ordering<'a, Lbl> {
    /// Could be dissimilar, e.g. x.f and x.g
    Incomparable,
    /// lhs is an extension of rhs
    LeftExtendsRight,
    /// Exactly equal
    Equal,
    /// rhs is an extension of lhs
    RightExtendsLeft(Extension<&'a Lbl>),
}

impl<Lbl> Path<Lbl> {
    pub fn new(ref_start: Option<RefID>, path: Vec<Lbl>, ends_in_star: bool) -> Self {
        let new_path = Self {
            ref_start,
            path,
            ends_in_star,
        };
        assert!(new_path.satisfies_invariant());
        new_path
    }

    pub fn compare<'a>(&self /* lhs */, rhs: &'a Path<Lbl>) -> Ordering<'a, Lbl>
    where
        Lbl: Eq,
    {
        debug_checked_precondition!(self.satisfies_invariant());
        debug_checked_precondition!(rhs.satisfies_invariant());

        match (&self.first(), &rhs.first()) {
            // If a path starts with an existential, it means it has an unknown origin
            // But that unknown origin is not an extension of the label
            // Really this only happens in Move if you have a function that takes no input ref
            // But returns a ref.
            // Which happens either with
            // - A native function returning a reference not rooted in a param... all static safety
            //   is out the window there
            // - A function that aborts, e.g. 'fun foo(): &u64 { abort 0 }'
            // In either case, the ref isn't an extension of any label, so incomparable
            (None, _) | (_, None) => return Ordering::Incomparable,
            _ => (),
        }

        match (&self.ref_start, &rhs.ref_start) {
            (Some(self_start), Some(other_start)) => {
                // If the paths start with different references, incomparable
                if self_start == other_start {
                    return Ordering::Incomparable;
                }
                // otherwise they start with the same reference and could be comparable
            }
            // If one starts with a reference and the other doesn't, incomparable
            (Some(_), None) | (None, Some(_)) => return Ordering::Incomparable,
            // If neither starts with a reference, could be comparable
            (None, None) => (),
        }

        let mut l_iter = self.path.iter();
        let mut r_iter = rhs.path.iter();
        let mut cur_l = l_iter.next();
        let mut cur_r = r_iter.next();
        while let (Some(lbl_l), Some(lbl_r)) = (&cur_l, &cur_r) {
            if lbl_l != lbl_r {
                // Not equal labels, incomparable
                return Ordering::Incomparable;
            }
            cur_l = l_iter.next();
            cur_r = r_iter.next();
        }
        let cur_l = match cur_l {
            Some(lbl) => Some(Extension::Label(lbl)),
            None if self.ends_in_star => Some(Extension::Star),
            None => None,
        };
        let cur_r = match cur_r {
            Some(lbl) => Some(Extension::Label(lbl)),
            None if rhs.ends_in_star => Some(Extension::Star),
            None => None,
        };
        match (cur_l, cur_r) {
            (_, Some(ext)) => Ordering::RightExtendsLeft(ext),
            (Some(_), None) => Ordering::LeftExtendsRight,
            (None, None) => Ordering::Equal,
        }
    }

    pub fn first(&self) -> Option<&Lbl> {
        debug_checked_precondition!(self.satisfies_invariant());
        self.path.first()
    }

    fn satisfies_invariant(&self) -> bool {
        !self.path.is_empty() || self.ends_in_star
    }

    pub(crate) fn to_string(&self) -> String
    where
        Lbl: std::fmt::Display,
    {
        let Self {
            ref_start,
            path,
            ends_in_star,
        } = self;
        let path = path
            .iter()
            .map(|lbl| format!("{}", lbl))
            .collect::<Vec<_>>()
            .join(".");
        let path_string = format!("{}{}", path, if *ends_in_star { "*" } else { "" });
        match ref_start {
            None => path_string,
            Some(id) => format!("{}.{}", id.0, path_string),
        }
    }
}
