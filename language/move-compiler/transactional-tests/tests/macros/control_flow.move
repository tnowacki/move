//# publish
module 0x1::strange {

public macro fun just_0(): u64 {
    0
}

public macro fun return_0(): u64 {
    if (true) return 0
    else 0
}

public macro fun for(start: u64, stop: u64, $body: |u64|) {
    let i = start;
    while (i < stop) {
        let cur = i;
        i = i + 1;
        $body(cur);
    }
}

}

//# publish
module 0x1::ex {

use 0x1::strange::{just_0, return_0, for};

entry fun example_1(): u64 {
    assert!(just_0!() != 0, 0);
    42
}

entry fun example_2(): u64 {
    assert!(return_0!() != 0, 0);
    42
}

entry fun i_like_this(): u64 {
    let count = 0;
    for!(0, 10, |i| {
        if (i % 2 == 0) continue;
        count = count + i;
    });
    count
}

entry fun i_like_this_alot<T>(v: &vector<T>, target: &T): u64 {
    let n = std::vector::length(v);
    let result = n;
    for!(0, n, |i| {
        if (std::vector::borrow(v, i) == target) {
            result = i;
            break
        }
    });
    result
}

}

//# run 0x1::ex::example_1

//# run 0x1::ex::example_2

//# run 0x1::ex::i_like_this

//# run 0x1::ex::i_like_this_alot --args vector[0,10,100,1000] 100 --type-args u64
