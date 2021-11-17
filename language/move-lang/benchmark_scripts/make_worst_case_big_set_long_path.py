#!/usr/bin/python

import sys

R = int(sys.argv[1])
F = int(sys.argv[2])
M = int(sys.argv[3])
assert R > 0
assert F > 0

print("// generated with stress_test.py")
print("// R = {}".format(R))
print("// F = {}".format(F))
print("// M = {}".format(M))

print("module 0x42.Example {")
print("")
# structs
zero_fields = ""
for f in range(0, F):
    if F == 1:
        comma = ""
    else:
        comma = ",\n"
    zero_fields += "        f{}: u64{}".format(f, comma)
print("    struct S0 has drop {\n" + zero_fields + "    }")
for r in range(1, R):
    prev = r - 1
    fields = ""
    for f in range(0, F):
        if F == 1:
            comma = ""
        else:
            comma = ",\n"
        fields += "        f{}: Self.S{}{}".format(f, prev, comma)
    print("    struct S{} has drop {{\n".format(r) + fields + "    }")
print("")

# function
Last = R - 1
print(
    "    worst_case_big_set_long_path(l{Last}: &mut Self.S{Last}, cond: bool) {{".format(Last=Last)
)
for r in reversed(range(0, Last)):
    print("        let l{r}: &mut Self.S{r};".format(r=r))
print("        let big_set: &mut u64;")
for m in range(0, M):
    print("        let extra{}: &mut u64;".format(m))
print("")

for prev in reversed(range(0, R)):
    if prev == 0:
        local = "big_set"
    else:
        local = "l{}".format(prev - 1)

    print("        {local} = &mut copy(l{prev}).S{prev}::f0;".format(local=local, prev=prev))
    elses = 0
    for f in range(1, F):
        if f == 1:
            maybe_else = ""
        else:
            maybe_else = "else { "
            elses += 1
        print(
            "        {e}if (copy(cond)) {{ {local} = &mut copy(l{prev}).S{prev}::f{f}; }}".format(
                e=maybe_else, local=local, prev=prev, f=f
            )
        )
    if elses > 0:
        close_elses = "        "
        for _ in range(elses):
            close_elses += "}"
        print(close_elses)
    print("")

for m in range(0, M):
    print("        extra{} = copy(big_set);".format(m))

print("        *move(big_set) = 0;")
for r in reversed(range(0, R)):
    print("        _ = move(l{});".format(r))
print("        *move(big_set) = 0;")
print("        return;")


print("    }")

print("")
print("}")
