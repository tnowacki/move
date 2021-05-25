module 0x42::M {
    struct X has drop { f: u64 }
    struct S has drop { x: X }

    fun t(cond: bool) {
        let s1 = S { x: X { f: 0 } };
        let s2 = S { x: X { f: 0 } };
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
