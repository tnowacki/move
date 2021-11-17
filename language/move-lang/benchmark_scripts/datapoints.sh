#!/bin/zsh

alias analyze="pkill cargo; SET_BASED=1 cargo run --quiet --release --bin time-ref-safety -- "
# alias analyze="pkill cargo; SET_BASED=1 cargo run --bin time-ref-safety -- "

MAX_R=4
R_STEP=1
INIT_R=1
R=$INIT_R

MAX_F=14
F_STEP=1
INIT_F=1
F=$INIT_F

MAX_M=50
M_STEP=10
INIT_M=0
M=$INIT_M

MAX_LOCALS=256

DIR=$HOME/diem/language/move-lang/benchmark_scripts

R=200
F=50
M=50

FILE=$DIR/sources/worst_case_big_set_long_path_R_${R}_F_${F}_M_${M}.mvir
if [ ! -f $FILE ]; then
    $DIR/make_worst_case_big_set_long_path.py $R $F $M &> $FILE
fi
analyze -m "R=$R, F=$F, M=$M" -n 1 -- $FILE

# echo "worst_case_big_set"
# R=1
# while [[ $R -le $MAX_R ]]
# do
#     FILE=$DIR/sources/worst_case_big_set_R_${R}_M_${M}.mvir
#     if [ ! -f $FILE ]; then
#         $DIR/make_worst_case_big_set.py $R $M &> $FILE
#     fi
#     analyze -m "M=$M, R=$R" -n 1 -- $FILE
#     if [[ $R == 1 ]]; then
#         R=$R_STEP
#     else
#         R=$(( $R + $R_STEP ))
#     fi
# done
# echo ""

# echo "worst_case_long_path"
# F=1
# while [[ $F -le $MAX_F ]]
# do
#     R=$(( $F + 1 ))
#     FILE=$DIR/sources/worst_case_long_path_R_${R}_F_${F}.mvir
#     if [ ! -f $FILE ]; then
#         $DIR/make_worst_case_long_path.py $F &> $FILE
#     fi
#     analyze -m "R=$R, F=$F" -n 1 -- $FILE
#     if [[ $F == 1 ]]; then
#         F=$F_STEP
#     else
#         F=$(( $F + $F_STEP ))
#     fi
# done

# M=$INIT_M
# R=$INIT_R
# while [[ $M -le $MAX_M ]]; do
#     F=$INIT_F
#     while [[ $F -le $MAX_F ]]; do
#         FILE=$DIR/sources/worst_case_big_set_long_path_R_${R}_F_${F}_M_${M}.mvir
#         if [ ! -f $FILE ]; then
#             $DIR/make_worst_case_big_set_long_path.py $R $F $M &> $FILE
#         fi
#         analyze -m "R=$R, F=$F, M=$M" -n 1 -- $FILE

#         if [[ $F == 1 ]] && [[ $F_STEP -gt 1 ]]; then
#             F=$F_STEP
#         else
#             F=$(( $F + $F_STEP ))
#         fi
#     done

#     if [[ $M == 0 ]] && [[ $M_STEP -gt 1 ]]; then
#         M=$M_STEP
#     else
#         M=$(( $M + $M_STEP ))
#     fi
# done

# M=$INIT_M
# F=$INIT_F
# NUM_LOCALS=$(($R + $M + 1))
# while [[ $M -le $MAX_M ]]; do
#     R=$INIT_R
#     while [[ $R -le $MAX_R  ]] && [[ $NUM_LOCALS -lt $MAX_LOCALS ]]; do
#         FILE=$DIR/sources/worst_case_big_set_long_path_R_${R}_F_${F}_M_${M}.mvir
#         if [ ! -f $FILE ]; then
#             $DIR/make_worst_case_big_set_long_path.py $R $F $M &> $FILE
#         fi
#         analyze -m "R=$R, F=$F, M=$M" -n 1 -- $FILE

#         if [[ $R == 1 ]] && [[ $R_STEP -gt 1 ]]; then
#             R=$R_STEP
#         else
#             R=$(( $R + $R_STEP ))
#         fi
#     done

#     if [[ $M == 0 ]] && [[ $M_STEP -gt 1 ]]; then
#         M=$M_STEP
#     else
#         M=$(( $M + $M_STEP ))
#     fi
# done

# M=$INIT_M
# F=2
# R=$INIT_R
# NUM_LOCALS=$(($R + $M + 1))
# while [[ $R -le $MAX_R  ]] && [[ $NUM_LOCALS -lt $MAX_LOCALS ]]; do
#     FILE=$DIR/sources/worst_case_big_set_long_path_R_${R}_F_${F}_M_${M}.mvir
#     if [ ! -f $FILE ]; then
#         $DIR/make_worst_case_big_set_long_path.py $R $F $M &> $FILE
#     fi
#     analyze -m "R=$R, F=$F, M=$M" -n 1 -- $FILE

#     if [[ $R == 1 ]] && [[ $R_STEP -gt 1 ]]; then
#         R=$R_STEP
#     else
#         R=$(( $R + $R_STEP ))
#     fi
# done

# M=$INIT_M
# while [[ $M -le $MAX_M ]]; do
#     F=$INIT_F
#     while [[ $F -le $MAX_F ]]; do
#         R=$INIT_R
#         NUM_LOCALS=$(($R + $M + 1))
#         while [[ $R -le $MAX_R  ]] && [[ $NUM_LOCALS -lt $MAX_LOCALS ]]; do
#             FILE=$DIR/sources/worst_case_big_set_long_path_R_${R}_F_${F}_M_${M}.mvir
#             if [ ! -f $FILE ]; then
#                 $DIR/make_worst_case_big_set_long_path.py $R $F $M &> $FILE
#             fi
#             analyze -m "R=$R, F=$F, M=$M" -n 1 -- $FILE

#             if [[ $R == 1 ]] && [[ $R_STEP -gt 1 ]]; then
#                 R=$R_STEP
#             else
#                 R=$(( $R + $R_STEP ))
#             fi
#         done

#         if [[ $F == 1 ]] && [[ $F_STEP -gt 1 ]]; then
#             F=$F_STEP
#         else
#             F=$(( $F + $F_STEP ))
#         fi
#     done

#     if [[ $M == 0 ]] && [[ $M_STEP -gt 1 ]]; then
#         M=$M_STEP
#     else
#         M=$(( $M + $M_STEP ))
#     fi
# done
