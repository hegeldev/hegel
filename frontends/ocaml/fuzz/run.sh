#!/bin/zsh
# usage: fuzz/run.sh <dir> <hours> <workers>
# Runs <workers> fuzzers for <hours>; each restarts after a failure until time is up
# or <dir>/stop exists.
set -e
L=${1:a}; hours=$2; workers=$3
root=${0:a:h:h}
mkdir -p $L/known
rm -f $L/stop
cd $root
dune build fuzz/ocaml_diff.exe
# a copy, so rebuilding doesn't disturb the run; replace it to update the workers as they restart
cp _build/default/fuzz/ocaml_diff.exe $L/fuzz.new && chmod +w $L/fuzz.new && mv -f $L/fuzz.new $L/fuzz.exe
set +e
echo $$ > $L/run.pid
end=$(( $(date +%s) + hours * 3600 ))
for w in $(seq 1 $workers); do
  ( i=0; while [ $(date +%s) -lt $end ] && [ ! -e $L/stop ]; do i=$((i+1))
      cd $root || exit 1
      OCAML_DIFF_SAVE=$L/known $L/fuzz.exe 1000000 > $L/w$w-$i.log 2>&1
    done ) &
done
# shortcut: SIGTERM skips the fuzzer's at_exit summary, so the last log of each worker has no "ran N" line
while [ $(date +%s) -lt $end ] && [ ! -e $L/stop ]; do sleep 10; done
pkill -f "$L/fuzz.exe"; wait; rm -f $L/run.pid
