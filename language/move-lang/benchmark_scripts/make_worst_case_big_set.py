#!/usr/bin/python

import sys

R = int(sys.argv[1])
M = int(sys.argv[2])
assert R > 0
assert M > 0

print("// R = {}".format(R))
print("// F = {}".format(1))

print("module 0x42.Example {")
print("")
# structs
for i in range(0, R):
    print("    struct S{} has drop {{ f: u64 }}".format(i))
print("")

# function
print("    worst_case_big_set(cond: bool) {")
print("        let big_set: &mut u64;")
for i in range(0, R):
    print("        let l{i}: Self.S{i};".format(i=i))
for i in range(0, M):
    print("        let x{i}: &mut u64;".format(i=i))

for i in range(0, R):
    print("        l{i} = S{i} {{ f: 0 }};".format(i=i))
print("")
print("        big_set = &mut (&mut l0).S0::f;")
elses = 0
for i in range(1, R):
    if i > 1:
        elses += 1
        maybeElse = "else { "
    else:
        maybeElse = ""
    print(
        "        {}if (copy(cond)) {{ big_set = &mut (&mut l{i}).S{i}::f; }}".format(maybeElse, i=i))
closing = "        "
for _ in range(elses):
    closing = closing + "}"
print(closing)
print("")
for i in range(0, M):
    print("        x{i} = copy(big_set);".format(i=i))
print("")
print("        *move(big_set) = 0;")
print("        return;")


print("    }")

print("")
print("}")
