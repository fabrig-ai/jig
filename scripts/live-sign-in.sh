#!/usr/bin/env bash
# The person-driven live layer (protocol/service.md, section 8): a fresh
# ChatGPT sign-in, a real model's answer and a confirmed sign-out.
set -euo pipefail
umask 077
cd "$(dirname "$0")/.."
# The machine's build queue where it has one, plain cargo where it hasn't
# (agents' docs/design/build-infrastructure.md 12).
heavy=$(command -v heavy || true)
printf '%s\n' 'The browser must reach 127.0.0.1:1455 on this machine: ssh -L 1455:127.0.0.1:1455 HOST'
export NEXTEST_DOUBLE_SPAWN=0
# shellcheck disable=SC2086
$heavy cargo nextest run -p jig-test --profile live --no-run
# The test's scratch goes under this script's own directory, so that a run cut
# short (an interrupt, nextest's timeout) leaves no secret behind: the
# secrets' stores and the agent trace are erased however it ends.
scratch=$(readlink -f "$(mktemp -d)")
runner=
stop() {
    : > "$scratch/stop-runner"
    [[ -z $runner ]] || wait "$runner" 2>/dev/null || true
}
erase() {
    [[ -d $scratch ]] || return 0
    stop
    find "$scratch" \( -type d -name secrets -prune -o -type f -name agent-trace.jsonl \) -exec rm -rf {} +
}
trap erase EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
status=0
TMPDIR=$scratch python3 scripts/owned-process.py --stop-file "$scratch/stop-runner" -- cargo nextest run -p jig-test --profile live --no-capture \
    -E 'test(=a_person_signs_in_to_chatgpt_and_a_real_model_answers)' &
runner=$!
wait "$runner" || status=$?
runner=
erase
if ((status == 0)); then
    rm -rf "$scratch"
else
    printf '%s\n' "Diagnostics, without secrets: $scratch" \
        "A sign-in that did not sign out may still be listed at the issuer. Disconnect it in ChatGPT's settings, under Apps."
fi
exit "$status"
