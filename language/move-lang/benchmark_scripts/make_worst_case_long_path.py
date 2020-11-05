#!/usr/bin/python

import sys

R = int(sys.argv[1])
assert R > 0

print("// R = {}".format(R))
print("// F = {}".format(1))

print("module 0x42.Example {")
print("")
# structs
print("    struct S0 has drop {{ f: u64 }}")
for r in range(1, R):
    prev = r - 1
    for f in range(0, F):
        fields += ", f{}: S{}".format(i, prev)
    print("    struct S{} has drop {{ {} }}".format(r, fields))
print("")

# function
Last = R - 1
print(
    "    worst_case_big_set_long_path(l{Last}: &mut Self.S{Last}, cond: bool) {{".format(Last=Last)
)
for r in reversed(range(0, Last)):
    print("        let l{r}: &mut Self.S{r};".format(i=i))
print("        let big_set: &mut u64;")
for r in reversed(range(0, Last)):
    prev = r + 1
    print("        l{r} = &mut copy(l{prev}).S{prev}::f0;".format(r=r, prev=prev))


print("        root = &mut copy(l0).S0::f;")
print("")
print("        *move(root) = Leaf { f: 0 };")
print("        return;")


print("    }")

print("")
print("}")
