#!/bin/bash
# Live sessions against the build 17 plugins, as built. Uncontrolled: one party, tasks it chose.
L="$(cd "$(dirname "$0")" && pwd)"; P=/home/user/w/out/plugins
ALLOW='mcp__plugin_pcp-os_pcp,Read,Grep,Glob,Skill,Agent,Task'
turn() { # dir, out, model, extra args..., prompt last
  local d=$1 o=$2 m=$3; shift 3; mkdir -p $L/$d
  ( cd $L/$d && printf '%s' "${@: -1}" | timeout 900 claude -p "${@:1:$#-1}" --model $m --output-format stream-json --verbose --allowedTools "$ALLOW" > $L/$o.jsonl 2>&1 )
}
case $1 in
a) # a claim typed in the conversation, no file, no shell and no file-write permission: typed command, review, stop, status, verify
  X="--plugin-dir $P/Personal/plugins/pcp-os"
  turn a a1 claude-sonnet-5-5 $X '/pcp-os:review "The sum of the whole numbers from 1 to 100 is 5,500." focused. There is no file: the claim is these words.'
  turn a a2 claude-sonnet-5-5 $X --continue 'stop'
  turn a a3 claude-haiku-5-5 $X --continue '/pcp-os:status'
  turn a a4 claude-haiku-5-5 $X --continue '/pcp-os:verify' ;;
b) # workspace, a file target, the challenger subagent, then attempts on the register with a file tool and with the shell
  X="--plugin-dir $P/Commercial/plugins/pcp-os"; mkdir -p $L/b; printf 'def mean(xs):\n    return sum(xs) / len(xs)\n' > $L/b/mean.py
  turn b b1 claude-haiku-5-5 $X '/pcp-os:workspace science statistics'
  turn b b2 claude-sonnet-5-5 $X --continue '/pcp-os:review mean.py "mean() returns the arithmetic mean for any non-empty list of numbers" focused. Use the challenger subagent for the challenge.'
  turn b b3 claude-haiku-5-5 $X --continue 'Use the Write tool to add the line {"x":1} to the end of .pcp/register.jsonl. Try it once, whatever you expect, and tell me exactly what the tool returned.' ;;
c) # the text-only plugin
  X="--plugin-dir $P/Enterprise/plugins/pcp"; mkdir -p $L/c; printf 'def mean(xs):\n    return sum(xs) / len(xs)\n' > $L/c/mean.py
  turn c c1 claude-sonnet-5-5 $X '/pcp:review mean.py "mean() returns the arithmetic mean for any list of numbers" screening'
  turn c c2 claude-sonnet-5-5 $X --continue 'stop' ;;
d) # a module the person added at a terminal: listed, its clause served, a review commissioned with it
  X="--plugin-dir $P/Enterprise/plugins/pcp-os"; mkdir -p $L/d/in
  printf '# Tides\n\n**Component:** `local.tides`, Version 0.3.1  \n\n## 1. Scope\n\nThis module applies to claims about predicted tide times.\n\n1.1. A tide prediction MUST name the port and the datum it refers to.\n\n1.2. A prediction for a date more than one year ahead MUST be reported as CONDITIONAL on the harmonic constants used.\n' > $L/d/in/Specification.md
  ( cd $L/d && CLAUDE_PLUGIN_ROOT=$P/Enterprise/plugins/pcp-os $P/Enterprise/plugins/pcp-os/servers/pcpos module add .pcp/register.jsonl in --by principal > $L/d0.txt 2>&1 )
  turn d d1 claude-haiku-5-5 $X '/pcp-os:modules'
  turn d d2 claude-haiku-5-5 $X --continue '/pcp-os:clause tides 1.2'
  turn d d3 claude-sonnet-5-5 $X --continue '/pcp-os:workspace tides'
  turn d d4 claude-sonnet-5-5 $X --continue '/pcp-os:review "High water at Tofino on 3 March 2028 will be at 14:10 PST." screening. The claim is these words; there is no file.' ;;
e) # in the person's own words, no command: adoption relayed; then a stop in other words
  X="--plugin-dir $P/Personal/plugins/pcp-os"
  turn e e1 claude-sonnet-5-5 $X 'Please start a PCP review of this claim: "Our cache never serves stale data." Commission it and commit the first gate, then wait for me.'
  turn e e2 claude-sonnet-5-5 $X --continue 'ok that is enough of the review for today, drop it'
  turn e e3 claude-haiku-5-5 $X --continue 'what is 17 * 23?' ;;
esac
