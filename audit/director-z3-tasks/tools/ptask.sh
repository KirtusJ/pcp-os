#!/bin/bash
# ptask.sh <label> <timeout seconds> <command...>
# Run one command detached as a recorded task. It is admitted in the PCP register of this folder, run by
# async_run.py in a fresh directory under records/, and released when the runner has exited. Afterwards:
#   records/<label>-<time>.plan/DONE        the runner's exit code (0: ended as expected, inside its timeout)
#   records/<label>-<time>.plan/runner.txt  the runner's line for the task, and the release
#   records/<label>-<time>/ASYNC_RECORD.json and T/STDOUT.txt, T/STDERR.txt
# Admission reserves a place and release gives it back; neither shows that a process ran or how it ended.
# The runner's record shows that.
set -u
HERE=$(cd "$(dirname "$0")/.." && pwd)
label=$1; timeout=$2; shift 2
dir=$HERE/records/$label-$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p "$dir.plan"
python3 - "$dir.plan/plan.json" "$timeout" "$@" <<'PY'
import json, sys
json.dump({"tasks": [{"id": "T", "argv": sys.argv[3:], "timeout": int(sys.argv[2]), "expect_exit": 0}], "width": 1}, open(sys.argv[1], "w"))
PY
id=$(python3 "$HERE/tools/pmcp.py" acr_task_admit label="$label" 2>/dev/null | sed -n 's/^ADMITTED //p')
[ -n "$id" ] || echo "not admitted in the PCP register; recorded by the runner only" > "$dir.plan/runner.txt"
nohup bash -c "python3 '$HERE/tools/async_run.py' '$dir.plan/plan.json' '$dir' >> '$dir.plan/runner.txt' 2>&1; echo \$? > '$dir.plan/DONE'; [ -n '$id' ] && python3 '$HERE/tools/pmcp.py' acr_task_release id=$id >> '$dir.plan/runner.txt' 2>&1" >/dev/null 2>&1 &
echo "${id:-unadmitted} $dir"
