module 0x1::NonEmptyVec {
    struct NonEmptyVec<T> has copy, drop, store { inner: vector<T> }
    public fun new<T>(elem: T): NonEmptyVec<T> {
        NonEmptyVec { inner: 0x1::Vector::singleton(elem) }
    }

    public fun borrow<T>(v: &NonEmptyVec<T>, i: u64): &T {
        0x1::Vector::borrow(v, i)
    }

    public fun borrow_mut<T>(v: &mut NonEmptyVec<T>, i: u64): &mut T {
        0x1::Vector::borrow_mut(v, i)
    }
}

module 0x42::M {
    struct Trips {
        bucket1: NonEmptyVec<u64>,
        bucket2: NonEmptyVec<u64>,
        bucket3: NonEmptyVec<u64>
    }

    fun (cond: bool, s1: &mut S, s2: &mut S) {
        let imm;
        let mut_;
        if (cond) {
            imm = &s1.x.f;
            mut_ = &mut s2.x;
        } else {
            imm = &s2.x.f;
            mut_ = &mut s1.x;
        };
        *mut_ = X { f: 1 };
        assert(*imm == 0, 42);
    }

}
