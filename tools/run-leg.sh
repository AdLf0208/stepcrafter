#!/usr/bin/env bash
# Run one stepcrafter e2e leg through the slot registry with its own socket and scratch world.
#   run-leg.sh <log-name> [CONTROL|<SELFTEST value>]
# The log lands in ports/_shared/evidence/stepcrafter-e2e/e2e-<log-name>.log.
set -uo pipefail
CLONE=/home/adlf/Desktop/all_the_mods_10_rust_rewrite
name=${1:?need a log name}
mode=${2:-}
cd "$CLONE"
eval "$(ports/tools/launcher-game.sh env stepcrafter)"
extra=()
game=$GAME
case "$mode" in
    # The control leg is the whole script against the Nightly built without the package: it must
    # fail at the first block check, not merely run the in-script control and then PASS on the
    # modded game.
    CONTROL) game=$CONTROL_GAME; extra=(CONTROL_GAME=) ;;
    # The control has its own log (run-leg.sh control CONTROL). Launching two games in one leg is
    # flaky while other agents' e2e jobs load the box, so the main leg runs one game only.
    "") extra=(CONTROL_GAME=) ;;
    *) extra=(CONTROL_GAME= SELFTEST="$mode") ;;
esac
sock=$E2E_TMP/stepcrafter-$name.sock
ports/tools/build-slot.sh run --job W1-stepcrafter--$name --port ports/stepcrafter --step e2e -- \
    env GAME="$game" CTL="$CTL" RES="$RES" \
    E2E_TMP="$E2E_TMP" SOCK="$sock" "${extra[@]}" \
    bash "$CLONE/ports/stepcrafter/tools/e2e.sh" \
    > "$CLONE/ports/_shared/evidence/stepcrafter-e2e/e2e-$name.log" 2>&1
rc=$?
printf '%s exit=%d\n' "$name" "$rc"
return $rc 2>/dev/null || exit $rc
