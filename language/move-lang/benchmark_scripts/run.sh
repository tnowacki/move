#!/bin/zsh

alias analyze="SET_BASED=1 cargo run --release --quiet --bin paper-analyze -- "
# all commands intended to be run from the Diem root dir

echo "benchmark: starcoin"
analyze ~/move_benchmarks/starcoin/vm/stdlib/modules

echo "benchmark: mai"
analyze ~/move_benchmarks/mai/src/modules -d ~/move_benchmarks/starcoin/vm/stdlib/modules

echo "benchmark: blackhole"
analyze ~/move_benchmarks/blackhole/protocol/src/modules -d ~/move_benchmarks/starcoin/vm/stdlib/modules

echo "benchmark: alma"
analyze ~/move_benchmarks/alma-contract/src/modules -d ~/move_benchmarks/starcoin/vm/stdlib/modules

echo "benchmark: starswap"
analyze ~/move_benchmarks/starswap-core/src/modules -d ~/move_benchmarks/starcoin/vm/stdlib/modules

echo "benchmark: meteor"
analyze ~/move_benchmarks/meteor-contract/src/modules -d ~/move_benchmarks/starcoin/vm/stdlib/modules

echo "benchmark: taohe"
analyze ~/move_benchmarks/taohe/modules -d  ~/diem/language/move-stdlib/sources

echo "benchmark: stdlib"
analyze ~/diem/language/move-stdlib/sources

echo "benchmark: diem"
analyze ~/diem/diem-move/diem-framework/core/sources ~/diem/language/move-stdlib/sources

# MerkleDistributor claim
# RedPackage claim
