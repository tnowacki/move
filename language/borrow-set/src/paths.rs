// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

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
    Other,
    /// Exactly equal
    Equal,
    /// rhs is an extension of lhs
    /// x.f and x.f.g yields `Extension(g)`
    /// x.(exists p).g and x.f.g yields `Extension(f)`
    /// x.f.g and x.(exists p) yields `Extension((exists p))`
    Extension(&'a Offset<Instr, Lbl>),
}

impl<Instr, Lbl> Path<Instr, Lbl> {
    pub fn empty() -> Self {
        Self(vec![])
    }

    pub fn extend(&self, extension: Offset<Instr, Lbl>) -> Self
    where
        Instr: Copy,
        Lbl: Clone,
    {
        let mut new_path = self.0.clone();
        new_path.push(extension);
        Self(new_path)
    }

    pub fn extended_by<'a>(
        &self, /* lhs */
        rhs: &'a Path<Instr, Lbl>,
    ) -> Ordering<'a, Instr, Lbl>
    where
        Instr: Eq,
        Lbl: Eq,
    {
        let mut l_iter = self.0.iter();
        let mut r_iter = rhs.0.iter();
        while let (Some(l), Some(r)) = (l_iter.next(), r_iter.next()) {
            match (l, r) {
                // Equal cases, continue
                (Offset::Labeled(lbl_l), Offset::Labeled(lbl_r)) if lbl_l == lbl_r => continue,
                (Offset::Existential(ext_l), Offset::Existential(ext_r)) if ext_l == ext_r => {
                    continue
                }

                // An existential is pessimistically extended by anything and extends anything
                // It is equivalent to '.*' in regex terms
                (Offset::Existential(_), r) | (_, r @ Offset::Existential(_)) => {
                    return Ordering::Extension(r)
                }

                // Two distinct non equal labels, dissimilar
                (Offset::Labeled(lbl_l), Offset::Labeled(lbl_r)) => {
                    assert!(lbl_l != lbl_r);
                    return Ordering::Other;
                }
            }
        }
        match (l_iter.next(), r_iter.next()) {
            (Some(_), None) => Ordering::Other,
            (None, None) => Ordering::Equal,
            (None, Some(r)) => Ordering::Extension(r),
            (Some(_), Some(_)) => unreachable!(),
        }
    }
}
