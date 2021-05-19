// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0
use mirai_annotations::debug_checked_precondition;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Offset<Instr, Lbl> {
    Labeled(Lbl),
    Existential((Instr, usize)),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Path<Instr, Lbl>(pub(crate) Vec<Offset<Instr, Lbl>>);

pub enum Ordering<'a, Instr, Lbl> {
    /// Could be dissimilar, e.g. x.f and x.g
    /// Could be something else like, x.f.(exists p) and x.f
    Incomparable,
    /// lhs is an extension of rhs
    LeftExtendsRight,
    /// Exactly equal
    Equal,
    /// rhs is an extension of lhs
    /// x.f and x.f.g yields `Extension(g)`
    /// x.(exists p).g and x.f.g yields `Extension(f)`
    /// x.f.g and x.(exists p) yields `Extension((exists p))`
    RightExtendsLeft(&'a Offset<Instr, Lbl>),
}

impl<Instr, Lbl> Path<Instr, Lbl> {
    pub fn initial(offset: Offset<Instr, Lbl>) -> Self {
        Self(vec![offset])
    }

    pub fn extend(&self, extension: Offset<Instr, Lbl>) -> Self
    where
        Instr: Copy,
        Lbl: Clone,
    {
        debug_checked_precondition!(self.satisfies_invariant());
        let mut new_path = self.0.clone();
        new_path.push(extension);
        let np = Self(new_path);
        debug_checked_precondition!(np.satisfies_invariant());
        np
    }

    pub fn compare<'a>(&self /* lhs */, rhs: &'a Path<Instr, Lbl>) -> Ordering<'a, Instr, Lbl>
    where
        Instr: Eq,
        Lbl: Eq,
    {
        debug_checked_precondition!(self.satisfies_invariant());
        debug_checked_precondition!(rhs.satisfies_invariant());

        match (&self.0[0], &rhs.0[0]) {
            // If a path starts with an existential, it means it has an unknown origin
            // But that unknown origin is not an extension of the label
            // Really this only happens in Move if you have a function that takes no input ref
            // But returns a ref.
            // Which happens either with
            // - A native function returning a reference not rooted in a param... all static safety
            //   is out the window there
            // - A function that aborts, e.g. 'fun foo(): &u64 { abort 0 }'
            // In either case, the ref isn't an extension of any label, so incomparable
            (Offset::Existential(_), Offset::Labeled(_))
            | (Offset::Labeled(_), Offset::Existential(_)) => return Ordering::Incomparable,
            _ => (),
        }

        let mut l_iter = self.0.iter();
        let mut r_iter = rhs.0.iter();
        while let (Some(l), Some(r)) = (l_iter.next(), r_iter.next()) {
            match (l, r) {
                // Equal cases, continue
                (Offset::Labeled(lbl_l), Offset::Labeled(lbl_r)) if lbl_l == lbl_r => continue,
                (Offset::Existential(ext_l), Offset::Existential(ext_r)) if ext_l == ext_r => {
                    continue
                }

                // Two distinct non equal offsets, dissimilar
                (l @ Offset::Existential(_), r @ Offset::Existential(_))
                | (l @ Offset::Labeled(_), r @ Offset::Labeled(_)) => {
                    assert!(l != r);
                    return Ordering::Incomparable;
                }

                // An existential is pessimistically extended by anything and extends anything
                // It is equivalent to '.*' in regex terms
                (Offset::Existential(_), r @ Offset::Labeled(_))
                | (Offset::Labeled(_), r @ Offset::Existential(_)) => {
                    return Ordering::RightExtendsLeft(r)
                }
            }
        }
        match (l_iter.next(), r_iter.next()) {
            (Some(_), None) => Ordering::LeftExtendsRight,
            (None, None) => Ordering::Equal,
            (None, Some(r)) => Ordering::RightExtendsLeft(r),
            (Some(_), Some(_)) => unreachable!(),
        }
    }

    pub fn first(&self) -> &Offset<Instr, Lbl> {
        debug_checked_precondition!(self.satisfies_invariant());
        &self.0[0]
    }

    fn satisfies_invariant(&self) -> bool {
        !self.0.is_empty()
    }

    pub(crate) fn to_string(&self) -> String
    where
        Instr: std::fmt::Display,
        Lbl: std::fmt::Display,
    {
        self.0
            .iter()
            .map(|offset| match offset {
                Offset::Labeled(lbl) => format!("{}", lbl),
                Offset::Existential((instr, num)) => format!("exists#{}#{}", instr, num),
            })
            .collect::<Vec<_>>()
            .join(".")
    }
}
