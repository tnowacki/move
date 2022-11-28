//# publish
module 0x1::global_helpers {

use std::option::{Self, Option};

public macro fun maybe_move_from<T: key>(a: address): Option<T> {
    if (exists<T>(a)) option::some(move_from<T>(a)) else option::none()
}

public macro fun maybe_move_to<T: key>(s: &signer, v: Option<T>) {
    if (option::is_some(&v)) move_to<T>(s, option::destroy_some(v))
    else option::destroy_none(v)
}

public macro fun modify<T: key>(a: address, $f: |&mut T|) {
    if (exists<T>(a)) {
        let r = borrow_global_mut<T>(a);
        $f(r)
    }
}

public macro fun read<T: key>(a: address, $f: |&T|) {
    if (exists<T>(a)) {
        let r = borrow_global<T>(a);
        $f(r)
    }
}

}

//# publish
module 0x42::example {

use 0x1::global_helpers::{maybe_move_from, maybe_move_to, modify, read};

struct S has key { count: u64 }

fun example(s: &signer) acquires S {
    let a = std::signer::address_of(s);
    move_to(s, S { count: 0 });
    maybe_move_to!(s, maybe_move_from<S>!(a));
    modify!(a, |S { count }| *count = *count + 1);
    let x = 0;
    read!(a, |S { count }| x = *count);
    assert!(x != 0, 0);
}

}
