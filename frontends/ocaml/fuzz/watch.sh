#!/bin/bash
# usage: fuzz/watch.sh <dir>
# Waits for a run started by run.sh <dir> to find a failure and prints its log. Stops the
# run if there are more than 40 logs (workers failing straight away) or free disk drops
# below 10GB.
L=$(cd "$1" && pwd)
failed() { grep -l "^Exception:" "$L"/w*.log 2>/dev/null; }
stop() { touch "$L/stop"; pkill -f "$L/fuzz.exe"; echo "$1"; exit 1; }
until [ -n "$(failed)" ]; do
  kill -0 "$(cat "$L/run.pid" 2>/dev/null)" 2>/dev/null || { echo "run ended"; exit 0; }
  [ "$(ls "$L"/w*.log 2>/dev/null | wc -l)" -gt 40 ] && stop "too many logs, stopped run"
  df -k "$L" | awk 'NR==2{exit !($4 < 10*1024*1024)}' && stop "low disk, stopped run"
  sleep 30
done
failed
date
