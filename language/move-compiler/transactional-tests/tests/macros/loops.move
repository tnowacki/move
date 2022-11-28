//# publish

module 0x42::m {

macro public fun for(start: u64, stop: u64, $body: |u64|) {
    let i = start;
    while (i < stop) {
        $body(i);
        i = i + 1
    }
}

macro public fun for_each<T>(v: &vector<T>, $body: |&T|) {
    let i = 0;
    let n = std::vector::length(v);
    while (i < n) {
        $body(std::vector::borrow(v, i));
        i = i + 1
    }
}

}

//# run
script {
fun main() {
    let count = 0;
    0x42::m::for!(0, 10, |i| count = count + i*i);
    assert!(count == 285, 0);

    let es = vector[0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let sum = 0;
    0x42::m::for_each<u64>!(&es, |x| sum = sum + *x);
    assert!(sum == 45, 0);
}
}
