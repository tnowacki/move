module 0x42::M {
    struct X has drop { f: u64, g: u64 }
    struct S has drop { x: X }

    fun t1(cond: bool, s1: &mut S, s2: &mut S) {
        let imm;
        let mut_;
        if (cond) {
            imm = &s1.x.f;
            mut_ = &mut s2.x;
        } else {
            imm = &s2.x.f;
            mut_ = &mut s1.x;
        };
        *mut_ = X { f: 1, g: 2 };
        assert(*imm == 0, 42);
    }

}
