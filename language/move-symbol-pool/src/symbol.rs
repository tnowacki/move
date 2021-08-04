// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{pool::Entry, SYMBOL_POOL};
use serde::{de::Deserialize, ser::Serialize};
use std::{borrow::Cow, cmp::Ordering, fmt, num::NonZeroU64, ops::Deref};

/// Represents a string that has been cached.
///
/// A `Symbol` represents a pointer to string data that is owned by the global
/// symbol pool; it is not the string data itself. This enables this
/// representation to implement `Copy` and other traits that some string types
/// cannot.
///
/// The strings that `Symbol` types represent are added to the global cache as
/// the `Symbol` are created.
///
/// ```
///# use crate::move_symbol_pool::Symbol;
/// let s1 = Symbol::from("hi"); // "hi" is stored in the global cache
/// let s2 = Symbol::from("hi"); // "hi" is already stored, cache does not grow
/// assert_eq!(s1, s2);
/// ```
///
/// Use the method [`as_str()`] to access the string value that a `Symbol`
/// represents. `Symbol` also implements the [`Display`] trait, so it can be
/// printed as an ordinary string would. This makes it easier to use with
/// crates that print strings to a terminal, such as codespan.
///
/// ```
///# use crate::move_symbol_pool::Symbol;
/// let message = format!("{} {}",
///     Symbol::from("hello").as_str(),
///     Symbol::from("world"));
/// assert_eq!(message, "hello world");
/// ```
///
/// [`as_str()`]: crate::Symbol::as_str
/// [`Display`]: std::fmt::Display
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Symbol(NonZeroU64);

/// The `Tag` signifies what sort of Symbol it is
/// The `Tag` is stored in the lowest two bits of the Symbol's value
/// `Tag::Dynamic`: if the `Symbol` points into the Symbol pool, the lower bits will be zero for
///                 pointer alignment. So the tag value is 0
/// `Tag::Inline`: if the string for the `Symbol` has less than 7 characters, those characters can
///                be stuffed into the value of the symbol, along with the tag and the length. The
///                tag is set to 1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Tag {
    /// Tag for `Symbol`s that point into into the `SYMBOL_POOL`
    Dynamic = DYNAMIC_TAG,
    /// Tag for `Symbol's that can fit entirely in the symbol's value
    Inlined = INLINE_TAG,
}

/// Tag value for a symbol put into the `SYMBOL_POOL`
const DYNAMIC_TAG: u8 = 0b_00;
/// Tag value for an inlined symbol.
const INLINE_TAG: u8 = 0b_01;
/// Mask for the tag bits
const TAG_MASK: u64 = 0b_11;
/// Offset for the storage of the length of the string in the inlined case
const LEN_OFFSET: u64 = 4;
/// Mask for the storage of the string length
const LEN_MASK: u64 = 0xF0;
/// Maximum length of the string if inlined into the symbol's value
const MAX_INLINE_LEN: usize = 7;

impl Symbol {
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    fn tag(&self) -> Tag {
        let tag = (self.0.get() & TAG_MASK) as u8;
        match tag {
            DYNAMIC_TAG => Tag::Dynamic,
            _ => {
                debug_assert!(tag == (Tag::Inlined as u8));
                Tag::Inlined
            }
        }
    }
}

impl<'a> From<Cow<'a, str>> for Symbol {
    fn from(s: Cow<'a, str>) -> Self {
        let len = s.len();
        if len <= MAX_INLINE_LEN {
            let mut data: u64 = (INLINE_TAG as u64) | ((len as u64) << LEN_OFFSET);
            {
                let dest = inline_symbol_slice_mut(&mut data);
                dest[..len].copy_from_slice(s.as_bytes())
            }
            Symbol(NonZeroU64::new(data).expect("value of an inlined symbol cannot be null"))
        } else {
            let mut pool = SYMBOL_POOL.lock().expect("could not acquire lock on pool");
            let address = pool.insert(s).as_ptr() as u64;
            Symbol(NonZeroU64::new(address).expect("address of pooled symbol cannot be null"))
        }
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Self::from(Cow::Borrowed(s))
    }
}

impl From<String> for Symbol {
    fn from(s: String) -> Self {
        Self::from(Cow::Owned(s))
    }
}

impl Deref for Symbol {
    type Target = str;

    fn deref(&self) -> &str {
        match self.tag() {
            Tag::Dynamic => {
                let ptr = self.0.get() as *const Entry;
                let entry = unsafe { &*ptr };
                &entry.string
            }
            Tag::Inlined => {
                let len = (self.0.get() & LEN_MASK) >> LEN_OFFSET;
                let bytes = &inline_symbol_slice(&self.0)[..(len as usize)];
                unsafe { std::str::from_utf8_unchecked(bytes) }
            }
        }
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl Ord for Symbol {
    fn cmp(&self, other: &Symbol) -> Ordering {
        if self.0 == other.0 {
            Ordering::Equal
        } else {
            self.as_str().cmp(other.as_str())
        }
    }
}

impl PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Symbol) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for Symbol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.as_str().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Symbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Symbol::from(String::deserialize(deserializer)?))
    }
}

#[inline(always)]
fn inline_symbol_slice(x: &NonZeroU64) -> &[u8] {
    unsafe {
        let x: *const NonZeroU64 = x;
        let mut data = x as *const u8;
        // Lowest byte (first in little-endian, last in big-endian) is skipped as it stores the
        // tag
        if cfg!(target_endian = "little") {
            data = data.offset(1);
        }
        let len = 7;
        std::slice::from_raw_parts(data, len)
    }
}

#[inline(always)]
fn inline_symbol_slice_mut(x: &mut u64) -> &mut [u8] {
    unsafe {
        let x: *mut u64 = x;
        let mut data = x as *mut u8;
        // Lowest byte (first in little-endian, last in big-endian) is skipped as it stores the
        // tag
        if cfg!(target_endian = "little") {
            data = data.offset(1);
        }
        let len = 7;
        std::slice::from_raw_parts_mut(data, len)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        symbol::{Tag, MAX_INLINE_LEN},
        Symbol,
    };
    use std::mem::size_of;

    #[test]
    fn test_size() {
        // Assert that the size of a Symbol is fairly small. Since it'll be used
        // throughout the Move codebase, increases to this size should be
        // scrutinized.
        assert_eq!(size_of::<Symbol>(), size_of::<u64>());
    }

    #[test]
    fn test_tag() {
        let s = "bonjour le monde";
        for n in 0..=s.len() {
            let sym = Symbol::from(&s[..n]);
            assert_eq!(sym.len(), n);
            let expected_tag = if n <= MAX_INLINE_LEN {
                Tag::Inlined
            } else {
                Tag::Dynamic
            };
            assert_eq!(sym.tag(), expected_tag)
        }
    }

    #[test]
    fn test_from_different_strings_have_different_addresses() {
        let s1 = Symbol::from("hi");
        let s2 = Symbol::from("hello");
        assert_eq!(s1.tag(), Tag::Inlined);
        assert_eq!(s2.tag(), Tag::Inlined);
        assert_ne!(s1.0, s2.0);

        let s1 = Symbol::from("hi");
        let s2 = Symbol::from("bonjour le monde");
        assert_eq!(s1.tag(), Tag::Inlined);
        assert_eq!(s2.tag(), Tag::Dynamic);
        assert_ne!(s1.0, s2.0);

        let s1 = Symbol::from("bonjour!");
        let s2 = Symbol::from("bonjour le monde");
        assert_eq!(s1.tag(), Tag::Dynamic);
        assert_eq!(s2.tag(), Tag::Dynamic);
        assert_ne!(s1.0, s2.0);
    }

    #[test]
    fn test_from_identical_strings_have_the_same_address() {
        let s1 = Symbol::from("hi");
        let s2 = Symbol::from("hi");
        assert_eq!(s1.tag(), Tag::Inlined);
        assert_eq!(s2.tag(), Tag::Inlined);
        assert_eq!(s1.0, s2.0);

        let s1 = Symbol::from("bonjour le monde");
        let s2 = Symbol::from("bonjour le monde");
        assert_eq!(s1.tag(), Tag::Dynamic);
        assert_eq!(s2.tag(), Tag::Dynamic);
        assert_eq!(s1.0, s2.0);
    }
}
