#!/usr/bin/env bash
# In-game check for Step Crafter (the SDK package, 0.2.9).
#
# This package is a native `.hmod`: the launcher compiles it into the game
# (`ports/tools/launcher-game.sh build stepcrafter`), and the resulting binary carries the block,
# its block entity, its 27-slot inventory and the `/stepcrafter` commands. There is no `--dev-mod`
# folder: the content travels inside the same package.
#
# Usage:
#   GAME=<launcher-built horizon-client> CTL=<horizon-ctl of the same build> tools/e2e.sh
#   GAME=… CTL=… CONTROL_GAME=<game without the package> …   # adds the negative leg
#   GAME=… CTL=… SELFTEST=1 …                                # must FAIL (proves it can fail)
#
# The End arena (stone floor at y=63, machines at y=64, x from 200). Native mod commands are typed
# with `ctl chat "/cmd"` — the player-chat path that reaches a mod's command — rather than
# `ctl command`, which runs the console path. The stock is the machine's own inventory, so it is
# filled through the declared container (`ctl container`), which is also the way a hopper would.
#
# What the default run proves, in order:
#  - the package's block and item exist, and an unported sibling id (`step_crafter_manager`, in
#    the jar but blocked on the Refined Storage grid surface) is refused by `setblock` and `give`;
#  - the carried recipe (`ECE/AMA/EDE`) crafts one Step Crafter from its five ingredients in a
#    real table and consumes them, and the same items in another pattern craft nothing;
#  - a fresh machine reads `target=none min=- max=- stored=0 phase=idle cells=0 crafted=0`;
#  - `/stepcrafter target … minecraft:torch` resolves the torch's own crafting recipe into a
#    2-cell step; an item with no crafting recipe and an unknown id are each refused with a reason;
#  - `/stepcrafter configure` takes a band and refuses an inverted one;
#  - the band rule: with 8 coal and 8 sticks loaded and the band set to 8..16, the machine fills
#    the target 0 → 16 in steps of 4, consuming exactly 4 coal and 4 sticks (buffer before/after);
#  - it stops at the maximum and does not overshoot;
#  - `/stepcrafter take` moves the target back into the inventory;
#  - a stick-less step reports `not-enough-ingredients` and the restock resumes it — the run is
#    the hysteresis, not a one-shot;
#  - `/stepcrafter step` takes one step now, and refuses with no ingredients;
#  - the hysteresis over the minimum: band 64..128 with 32+32 loaded fills past `min` to `max`,
#    and the `active` state is true while it runs and false once idle;
#  - a broken machine's item carries its pattern and band and placing it back restores them, while
#    its stock spills as item entities (the engine's container spill);
#  - the Step Requester: a requester beside a Step Crafter whose own band never fires, filtering
#    torches 4..8 in batches of 4, makes that crafter step to 8 torches (`stored=8 crafted=8`),
#    and the *same world with the requester removed* leaves a second crafter at 0 — the ask is the
#    only difference, so the assertion can fail;
#  - the refusal and the retry: with the sticks drained the requester reports
#    `not-enough-ingredients`, and the restock resumes it;
#  - the requester's own `active` state follows its run;
#  - the requester keeps several filter entries: one requester with a torch entry and a stick entry
#    asks the torch crafter to 8 and the stick crafter to 16, reading `filters=2` with both entries
#    and their own stored counts, while the same world with only the torch entry leaves the stick
#    crafter at 0 — the removed entry is the only difference;
#  - a result that fits no slot is refused: a crafter whose 27 slots are 26 x 64 coal + 60 sticks
#    (1,724 of an aggregate 1,728) is asked for torches by a requester, and every ask is refused with
#    nothing consumed, nothing crafted and the ask not served (the free room after the ingredients
#    are taken is per slot, not the inventory's item count);
#  - a named stack of the target is no room for the step's plain result: the same 1,724-item machine
#    with 60 **named** torches refuses the step (network-full, nothing consumed, stock unchanged),
#    while the identical step beside a plain stack of 60 merges its four torches — the differential;
#  - take hands over the stored stacks themselves: two differently named, damaged swords are taken
#    and their custom names and damage are read back from the player's inventory;
#  - a step leaves what its ingredients leave: a cake step from 3 milk buckets, 2 sugar, an egg and
#    3 wheat makes the cake **and** three buckets, consuming every ingredient (the recipe's
#    remainders, which this port used to destroy);
#  - the requester's screen (0.2.6): a right click opens a 176 x 238 mod menu whose panel the
#    game reports at the centred origin, a carried torch clicked onto its first cell makes filter
#    entry 0, the drawn value buttons set that entry to `min=4 max=8 batch=4` (and the band's own
#    guard refuses `min +` at 0..0), a carried stick repoints the cell while keeping its band, a
#    command-added entry appears as a cell the remove button drops, and a requester nobody opened
#    carries no entry — the screen and `/stepcrafter requester` are one saved container;
#  - the machine's own panel (0.2.8): a right click opens the package's own 176 x 208 menu type with
#    the 27 stock slots still there at 300..326, a carried torch clicked onto the drawn target cell
#    makes `/stepcrafter info` read `target=minecraft:torch cells=2 result=minecraft:torch:4`, the
#    drawn `max +` x16 and `min +` x8 set the band to `min=8 max=16`, a `min +` that would invert
#    the band at `8..8` is refused, and a machine nobody opened reads `target=none min=- max=-`;
#  - the machine's several patterns (0.2.9): two cells pressed on its panel make two saved slots,
#    each with its own band, and both fill out of the shared stock (`targets=2`, torch 8..16 and
#    stick 16..32, four coal and sixteen planks consumed), while the same spot with only the first
#    cell pressed reads `targets=1` with one entry — the second cell is the only difference;
#  - two requesters share one crafter: the ask is now a list in the crafter's compound, so the
#    second is queued behind the first instead of being lost, each reads its own share of the stock
#    (the first `stored=4 served=4`, the second `stored=0 queued=1`), and a restock serves the second
#    its own four;
#  - `/stepcrafter` needs an operator: a non-operator's `info` is refused.
#
# SELFTEST=1 flips the requester's headline assertion (`stored=8`), so the run must end `EXIT:1`;
# SELFTEST=crafter flips the machine's first "filled to the maximum" assertion instead;
# SELFTEST=capacity flips the no-room assertion (nothing fit, so nothing was served);
# SELFTEST=components-room flips the component-room assertion (the named stack counted no room and
# the step was refused, so the fixed package must fail an inverted run);
# SELFTEST=take flips the component assertion (a stack rebuilt from its id reads no components);
# SELFTEST=remainder flips the remainder assertion (today's defect: the buckets are destroyed);
# SELFTEST=filters flips the multi-entry assertion (today's defect: one entry, so `filters=1`);
# SELFTEST=screen flips the screen assertion (today's defect: the right click opens no screen, so
# `ctl screen` reads `playing` and no entry can be made from a menu);
# SELFTEST=queue flips the queued-ask assertion (today's defect: the ask is one `Int`, so a second
# requester asking the same crafter is never queued and reads the first's torches as its own);
# SELFTEST=crafterscreen flips the machine-panel assertion (today's defect: the panel has no target
# cell, so the carried torch clicked on it changes nothing and the target stays `none`);
# SELFTEST=patterns flips the several-patterns assertion (today's defect: one target for the whole
# machine, `targets=1`, even after both cells were pressed).
# CONTROL_GAME is the same Nightly built **without** this package: the block and item ids must then
# be unknown, so the scenario is not vacuous.
set -uo pipefail
export LC_ALL=C

CLONE=/home/adlf/Desktop/all_the_mods_10_rust_rewrite
PORT=$CLONE/ports/stepcrafter
CTL=${CTL:-}
GAME=${GAME:-}
CONTROL_GAME=${CONTROL_GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
# `/tmp` is a 7.5 GB tmpfs on this box: honour the harness's own scratch root.
E2E_TMP=${E2E_TMP:-/tmp/opencode}
mkdir -p "$E2E_TMP"
SOCK=${SOCK:-$E2E_TMP/stepcrafter-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=StepCrafterE2E
CONTROL_WORLD=StepCrafterControl
BLOCK=stepcrafter:step_crafter
REQUESTER=stepcrafter:step_requester

[ -n "$CTL" ] || { echo "set CTL to the horizon-ctl built for the same build as GAME" >&2; exit 2; }
[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -n "$GAME" ] || { echo "set GAME to a game the launcher built with this package" >&2; exit 2; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
if [ -n "$CONTROL_GAME" ] && [ ! -x "$CONTROL_GAME" ]; then
    echo "missing CONTROL_GAME: $CONTROL_GAME" >&2
    exit 1
fi
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d "$E2E_TMP/stepcrafter-home.XXXXXX")
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" --timeout 60 "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }
abort() { echo "FAIL: $*" >&2; exit 1; }

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch [WORLD] [GAME]
    local world=${1:-$WORLD}
    local game=${2:-$GAME}
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" --timeout 180 launch --game "$game" --world "$world" --seed 20261105 \
        --creative --offscreen --wait --resources "$RES" >/dev/null
    ctl wait-ready 120
    ctl wait 40
}

# A stone floor and a clear volume in The End, so the arena is deterministic.
arena() {
    ctl command "execute in minecraft:the_end run tp @s 200.5 64 3.5"
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5
        if [ "$(ctl block 200 63 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill 192 63 -8 226 63 8 minecraft:stone"
    ctl command "execute in minecraft:the_end run fill 192 64 -8 226 74 8 minecraft:air"
    ctl command "execute in minecraft:the_end run kill @e[type=!player]"
    ctl command "gamemode creative"
    ctl wait 20
}

# The `/stepcrafter info` line for a position (the harness readout).
info() { # X Y Z
    say "stepcrafter info $1 $2 $3"
    ctl wait 8 >/dev/null
    ctl messages 20 | jq -r '.chat[]' | grep -E "^stepcrafter $1 $2 $3 " | head -1 || true
}

# The newest chat line matching a pattern (a command's parsed reply).
reply() { # PATTERN
    ctl wait 8 >/dev/null
    ctl messages 20 | jq -r '.chat[]' | grep -E "$1" | head -1 || true
}

expect() { # X Y Z TEXT
    local line
    line=$(info "$1" "$2" "$3")
    echo "  info($1,$2,$3): $line"
    case "$line" in
        *"$4"*) ;;
        *) fail "expected '$4' in '$line'" ;;
    esac
}

inventory_count() { # ITEM
    ctl inventory | jq -r --arg item "$1" '.counts[$item] // 0'
}

# Load the machine's inventory through its declared container, which is also the hopper path.
load() { # X Y Z ITEM COUNT
    ctl container "$1" "$2" "$3" "$4" "$5" >/dev/null || fail "cannot put $5 x $4 into the machine"
    ctl wait 6
}

box() { # X Y Z
    ctl container "$1" "$2" "$3" | jq -c '[.slots[] | select(. != null) | {item, count}]'
}

# Plant the 27 slots of the component-room machines directly in the block entity, through the
# pin's `/data merge block` path: a stack of the target (named or plain) with 4 of nominal room and
# the torch recipe's ingredients filling every other slot (so no slot is empty). 25 coal stacks plus
# the stick are the recipe's two cells; the brief's literal 26 coal stacks leave no stick, so the
# step would stop at `not-enough-ingredients` before the room check this leg is about.
plant_room() { # X Y Z named|plain
    local torch
    if [ "$4" = named ]; then
        torch='{Slot:0b,id:"minecraft:torch",count:60,components:{"minecraft:custom_name":"Kept"}}'
    else
        torch='{Slot:0b,id:"minecraft:torch",count:60}'
    fi
    local items=$torch slot
    for slot in $(seq 1 25); do
        items="$items,{Slot:${slot}b,id:\"minecraft:coal\",count:64}"
    done
    items="$items,{Slot:26b,id:\"minecraft:stick\",count:64}"
    ctl command "execute in minecraft:the_end run data merge block $1 $2 $3 {Items:[$items]}" >/dev/null
    ctl wait 6
}

# Whether a block's `active` state is `true`/`false` right now: the game's own predicate, so the
# answer is parsed from chat (`Test passed` / `Test failed`), not guessed from the id.
active_is() { # X Y Z true|false [BLOCK]
    ctl command "execute if block $1 $2 $3 ${5:-$BLOCK}[active=$4]" >/dev/null
    ctl wait 6 >/dev/null
    ctl messages 6 | jq -r '.chat[]' | grep -m1 -E 'Test (passed|failed)' || true
}

# CONTROL_GAME: the same scenario on a game built without the package, where the block and item
# ids must be unknown.
control_leg() {
    echo "== control: the same game without this package =="
    launch "$CONTROL_WORLD" "$CONTROL_GAME"
    arena
    ctl command "execute in minecraft:the_end run setblock 200 64 0 $BLOCK" >/dev/null 2>&1 || true
    ctl wait 4
    local name
    name=$(ctl block 200 64 0 | jq -r '.block')
    [ "$name" = "minecraft:air" ] || fail "the control game holds $name, so the block id was known"
    ctl give "$BLOCK" 1 >/dev/null 2>&1 || true
    ctl wait 4
    ctl inventory | jq -e --arg item "$BLOCK" '[.hotbar[], .main[] | select(.item == $item)] | length == 0' >/dev/null \
        || fail "the control game has a Step Crafter item, so the package was in it"
    say "stepcrafter info 200 64 0" >/dev/null 2>&1 || true
    ctl wait 6
    echo "  without the package: no block at 200 64 0 ($name), no item, no command"
    ctl quit >/dev/null 2>&1 || true
}

echo "== the package carries upstream's licence and the port's own assets =="
[ -f "$PORT/stepcrafter.hmod" ] || abort "no package at $PORT/stepcrafter.hmod"
unzip -p "$PORT/stepcrafter.hmod" LICENSE >/dev/null 2>&1 || abort "the package carries no LICENSE"
# List to a file, not `| grep -q`: under `set -o pipefail` a grep that exits on its match can
# fail the pipeline on the writer's SIGPIPE (`unzip` did once, on 2026-11-23), which reads as a
# missing model.
unzip -l "$PORT/stepcrafter.hmod" > "$SCRATCH/package-entries.txt" 2>/dev/null \
    || abort "the package cannot be listed"
grep -q 'step_crafter_inactive.json' "$SCRATCH/package-entries.txt" \
    || abort "the package carries no block model"
echo "  LICENSE and the block model are inside the archive"

echo "== launch (The End arena, the package built into the game) =="
if [ -n "$CONTROL_GAME" ]; then control_leg; fi
launch
arena

echo "== the block and item exist; an unported sibling id is refused =="
ctl command "execute in minecraft:the_end run setblock 200 64 0 $BLOCK"
ctl wait 6
block=$(ctl block 200 64 0 | jq -r '.block')
echo "  block(200,64,0): $block"
case "$block" in
    "$BLOCK"*) ;;
    *) fail "expected $BLOCK at 200 64 0, got $block" ;;
esac
ctl give "$BLOCK" 1
ctl wait 4
ctl inventory | jq -e --arg item "$BLOCK" '[.hotbar[] | select(.item == $item)] | length >= 1' >/dev/null \
    || fail "the Step Crafter item did not register"
# `step_crafter_manager` is in the jar and NOT in this slice (blocked on the RS grid surface).
UNKNOWN=stepcrafter:step_crafter_manager
ctl command "execute in minecraft:the_end run setblock 204 64 6 $UNKNOWN" >/dev/null 2>&1 || true
ctl wait 4
name=$(ctl block 204 64 6 | jq -r '.block')
echo "  block(204,64,6) for $UNKNOWN: $name"
[ "$name" = "minecraft:air" ] || fail "$UNKNOWN became a block, so an unported id was registered"
if ctl give "$UNKNOWN" 1 >/dev/null 2>&1; then
    count=$(inventory_count "$UNKNOWN")
    [ "$count" = 0 ] || fail "$UNKNOWN was given, so an unported id was registered"
fi
echo "  the unported manager id is refused by setblock and give"

echo "== the carried recipe crafts one Step Crafter and consumes its ingredients =="
ctl command "execute in minecraft:the_end run setblock 208 64 6 minecraft:crafting_table" >/dev/null
ctl wait 3
python3 "$PORT/tools/craft_check.py" "$SOCK" "$CTL" 208 64 6 || fail "the recipe check failed"
ctl wait 4
echo "  the recipe leg passed"

echo "== a fresh machine reads unconfigured =="
# A new machine at 202 64 0, so the recipe leg's inventory is not disturbed.
ctl command "execute in minecraft:the_end run setblock 202 64 0 $BLOCK"
ctl wait 6
FRESH=$(info 202 64 0)
echo "  $FRESH"
case "$FRESH" in
    *"target=none min=- max=- stored=0 phase=idle cells=0 result=none crafted=0"*) ;;
    *) fail "a fresh machine does not read unconfigured: $FRESH" ;;
esac
[ "$(box 202 64 0)" = "[]" ] || fail "a fresh machine's inventory is not empty: $(box 202 64 0)"

echo "== /stepcrafter target resolves the item's crafting recipe =="
say "stepcrafter target 202 64 0 minecraft:torch"
TARGET=$(reply '^stepcrafter target 202 64 0 ')
echo "  $TARGET"
case "$TARGET" in
    *"target=minecraft:torch cells=2 result=minecraft:torch:4"*) ;;
    *) fail "the torch target did not resolve to a 2-cell step: $TARGET" ;;
esac
expect 202 64 0 "target=minecraft:torch min=- max=- stored=0 phase=idle cells=2 result=minecraft:torch:4"
# The control: an item with no crafting recipe has no step (bedrock is not crafted at all).
say "stepcrafter target 202 64 0 minecraft:bedrock"
REFUSED=$(reply '^stepcrafter: minecraft:bedrock')
echo "  $REFUSED"
case "$REFUSED" in
    *"has no minecraft:crafting recipe"*) ;;
    *) fail "a bedrock target was not refused with a reason: $REFUSED" ;;
esac
expect 202 64 0 "target=minecraft:torch"
# An unknown id is a different refusal.
say "stepcrafter target 202 64 0 minecraft:not_a_thing"
UNKNOWN_REPLY=$(reply '^stepcrafter: minecraft:not_a_thing')
echo "  $UNKNOWN_REPLY"
case "$UNKNOWN_REPLY" in
    *"is not an item this world knows"*) ;;
    *) fail "an unknown target was not refused: $UNKNOWN_REPLY" ;;
esac

echo "== /stepcrafter configure takes a band and refuses an inverted one =="
say "stepcrafter configure 202 64 0 8 16"
BAND=$(reply '^stepcrafter configure 202 64 0 ')
echo "  $BAND"
case "$BAND" in
    *"min=8 max=16"*) ;;
    *) fail "the band was not configured: $BAND" ;;
esac
say "stepcrafter configure 202 64 0 9 4"
INVERTED=$(reply '^stepcrafter: min 9 is above max 4')
echo "  $INVERTED"
case "$INVERTED" in
    *"min 9 is above max 4"*) ;;
    *) fail "an inverted band was not refused: $INVERTED" ;;
esac
expect 202 64 0 "min=8 max=16"

echo "== the inventory is the stock, and the tick fills the target to the maximum =="
# The stock is loaded through the machine's own container: 8 coal and 8 sticks, then the band is
# set, so the "before" reading is the state the machine will start from.
say "stepcrafter configure 202 64 0 0 0"
reply '^stepcrafter configure 202 64 0 ' >/dev/null
load 202 64 0 minecraft:coal 8
load 202 64 0 minecraft:stick 8
BEFORE=$(info 202 64 0)
echo "  before the band: $BEFORE"
case "$BEFORE" in
    *"stored=0"*"minecraft:coal:8"*"minecraft:stick:8"*) ;;
    *) fail "the container did not hold 8 coal and 8 sticks with no target: $BEFORE" ;;
esac
echo "  container before: $(box 202 64 0)"
say "stepcrafter configure 202 64 0 8 16"
reply '^stepcrafter configure 202 64 0 ' >/dev/null
ctl wait 60
FILLED=$(info 202 64 0)
echo "  $FILLED"
if [ "$SELFTEST" = crafter ] || [ "$SELFTEST" = 2 ]; then
    # The machine's headline assertion, deliberately inverted: the machine did fill to the
    # maximum, so a run that must be able to fail has to end here, non-zero.
    case "$FILLED" in
        *"stored=16"*) fail "SELFTEST=crafter: the machine filled to 16 and the run must end non-zero: $FILLED" ;;
        *) ;;
    esac
else
    case "$FILLED" in
        *"stored=16"*) ;;
        *) fail "the machine did not fill the target to max 16: $FILLED" ;;
    esac
    case "$FILLED" in
        *"phase=idle"*) ;;
        *) fail "the machine did not stop at the maximum: $FILLED" ;;
    esac
    case "$FILLED" in
        *"crafted=16"*) ;;
        *) fail "the machine crafted a different total than 16: $FILLED" ;;
    esac
    case "$FILLED" in
        *"minecraft:coal:4"*"minecraft:stick:4"*) ;;
        *) fail "the four steps did not consume exactly 4 coal and 4 sticks: $FILLED" ;;
    esac
fi

echo "== it stops at the maximum: no overshoot =="
ctl wait 60
STEADY=$(info 202 64 0)
echo "  $STEADY"
case "$STEADY" in
    *"stored=16"*"crafted=16"*) ;;
    *) fail "the machine kept crafting past the maximum: $STEADY" ;;
esac

echo "== /stepcrafter take empties the target back into the inventory =="
BEFORE=$(inventory_count minecraft:torch)
say "stepcrafter take 202 64 0 minecraft:torch 8"
TAKEN=$(reply '^stepcrafter take 202 64 0 ')
echo "  $TAKEN"
case "$TAKEN" in
    *"stored=8 taken=8"*) ;;
    *) fail "take did not move 8 torches: $TAKEN" ;;
esac
ctl wait 6
AFTER=$(inventory_count minecraft:torch)
echo "  torches in the inventory: $BEFORE -> $AFTER"
[ "$AFTER" -ge $((BEFORE + 8)) ] || fail "the 8 taken torches did not reach the inventory"
expect 202 64 0 "stored=8"

echo "== too few ingredients: the machine reports it, and recovers when restocked =="
# Drain the sticks, then take the target below the minimum so a step is attempted.
say "stepcrafter take 202 64 0 minecraft:stick 4"
ctl wait 6
say "stepcrafter take 202 64 0 minecraft:torch 8"
ctl wait 6
ctl wait 40
STUCK=$(info 202 64 0)
echo "  $STUCK"
case "$STUCK" in
    *"stored=0"*"phase=not-enough-ingredients"*) ;;
    *) fail "a step with no sticks did not report not-enough-ingredients: $STUCK" ;;
esac
case "$STUCK" in
    *"crafted=16"*) ;;
    *) fail "the stuck machine crafted something: $STUCK" ;;
esac
load 202 64 0 minecraft:stick 4
ctl wait 60
RECOVERED=$(info 202 64 0)
echo "  $RECOVERED"
case "$RECOVERED" in
    *"stored=16"*"phase=idle"*) ;;
    *) fail "the restocked machine did not resume and fill to the maximum: $RECOVERED" ;;
esac
case "$RECOVERED" in
    *"crafted=32"*) ;;
    *) fail "the resumed machine crafted a different total than 32: $RECOVERED" ;;
esac

echo "== /stepcrafter step takes one step now =="
ctl command "execute in minecraft:the_end run setblock 206 64 0 $BLOCK"
ctl wait 6
say "stepcrafter target 206 64 0 minecraft:torch"
reply '^stepcrafter target 206 64 0 ' >/dev/null
say "stepcrafter configure 206 64 0 0 0"
reply '^stepcrafter configure 206 64 0 ' >/dev/null
load 206 64 0 minecraft:coal 1
load 206 64 0 minecraft:stick 1
# The band (0..0) holds: nothing is crafted until the step is asked for.
expect 206 64 0 "stored=0 phase=idle cells=2 result=minecraft:torch:4 crafted=0"
say "stepcrafter step 206 64 0"
STEPPED=$(reply '^stepcrafter step 206 64 0 ')
echo "  $STEPPED"
case "$STEPPED" in
    *"result=crafted"*) ;;
    *) fail "the manual step did not craft: $STEPPED" ;;
esac
expect 206 64 0 "stored=4 phase=idle cells=2 result=minecraft:torch:4 crafted=4"
# With no ingredients one more step reports the refusal instead of crafting, and records it in the
# machine's `phase` exactly as a tick-driven attempt does (0.2.4: a manual step is a step).
say "stepcrafter step 206 64 0"
STEP2=$(reply '^stepcrafter step 206 64 0 ')
echo "  $STEP2"
case "$STEP2" in
    *"result=not-enough-ingredients"*) ;;
    *) fail "a step with no ingredients was not refused: $STEP2" ;;
esac
expect 206 64 0 "stored=4 phase=not-enough-ingredients cells=2 result=minecraft:torch:4 crafted=4"

echo "== the run goes past the minimum to the maximum, and active follows it =="
ctl command "execute in minecraft:the_end run setblock 210 64 0 $BLOCK"
ctl wait 6
say "stepcrafter target 210 64 0 minecraft:torch"
reply '^stepcrafter target 210 64 0 ' >/dev/null
load 210 64 0 minecraft:coal 32
load 210 64 0 minecraft:stick 32
# No band yet, so the machine holds while the stock is loaded.
say "stepcrafter configure 210 64 0 64 128"
reply '^stepcrafter configure 210 64 0 ' >/dev/null
ctl wait 4
RUNNING=$(active_is 210 64 0 true)
echo "  active=true while filling: $RUNNING"
case "$RUNNING" in
    *"Test passed"*) ;;
    *) fail "the machine did not show active=true while filling: $RUNNING" ;;
esac
ctl wait 200
DONE=$(active_is 210 64 0 false)
echo "  active=false once filled: $DONE"
case "$DONE" in
    *"Test passed"*) ;;
    *) fail "the machine still shows active=true once idle: $DONE" ;;
esac
# 32 steps of 4 is 128: it ran past `min` (64) and stopped at `max` (128), the hysteresis.
expect 210 64 0 "stored=128 phase=idle cells=2 result=minecraft:torch:4 crafted=128"

echo "== the Step Requester asks a nearby Step Crafter (the interop) =="
# A machine whose own band never fires (0..0) and a requester beside it: every torch the machine
# makes from here on was asked for by the requester, not maintained by itself. The stock is loaded
# before the filter is set, so the first ask can be served.
ctl command "execute in minecraft:the_end run setblock 211 64 0 $BLOCK"
ctl wait 6
say "stepcrafter target 211 64 0 minecraft:torch"
reply '^stepcrafter target 211 64 0 ' >/dev/null
say "stepcrafter configure 211 64 0 0 0"
reply '^stepcrafter configure 211 64 0 ' >/dev/null
load 211 64 0 minecraft:coal 4
load 211 64 0 minecraft:stick 4
expect 211 64 0 "min=0 max=0 stored=0 phase=idle"
QUIET=$(info 211 64 0)
case "$QUIET" in
    *"crafted=0"*) ;;
    *) fail "the quiet machine crafted before the requester existed: $QUIET" ;;
esac
ctl command "execute in minecraft:the_end run setblock 212 64 0 $REQUESTER"
ctl wait 6
PLACED_REQ=$(ctl block 212 64 0 | jq -r '.block')
echo "  block(212,64,0): $PLACED_REQ"
[ "$PLACED_REQ" = "$REQUESTER" ] || fail "expected $REQUESTER at 212 64 0, got $PLACED_REQ"
ctl give "$REQUESTER" 1
ctl wait 4
ctl inventory | jq -e --arg item "$REQUESTER" '[.hotbar[] | select(.item == $item)] | length >= 1' >/dev/null \
    || fail "the Step Requester item did not register"
say "stepcrafter requester 212 64 0 minecraft:torch 4 8 4"
FILTER=$(reply '^stepcrafter requester 212 64 0 ')
echo "  $FILTER"
case "$FILTER" in
    *"item=minecraft:torch min=4 max=8 batch=4"*) ;;
    *) fail "the requester filter was not configured: $FILTER" ;;
esac
# A read right after the filter is set: the requester has found the crafter beside it and has
# already asked it (the two steps of this 4..8 run complete inside the harness's read latency, so
# `phase=running` is read on the 32-step run below, where the window is 32 ticks wide).
ctl wait 1
ASKING=$(info 212 64 0)
echo "  just after the filter: $ASKING"
case "$ASKING" in
    *"kind=requester"*"item=minecraft:torch"*"min=4 max=8 batch=4"*) ;;
    *) fail "the requester's readout is not the requester's: $ASKING" ;;
esac
case "$ASKING" in
    *"crafter=211,64,0"*) ;;
    *) fail "the requester did not link the crafter beside it: $ASKING" ;;
esac
case "$ASKING" in
    *"requests=0"*) fail "the requester linked the crafter but asked nothing: $ASKING" ;;
    *) ;;
esac
ctl wait 40
ASKED=$(info 212 64 0)
ASKCRAFT=$(info 211 64 0)
echo "  after 40 ticks: $ASKED"
echo "  the crafter:    $ASKCRAFT"
if [ "$SELFTEST" = 1 ] || [ "$SELFTEST" = requester ]; then
    # The new family's headline assertion, deliberately inverted: the requester did reach stored=8,
    # so a run that must be able to fail has to end here, non-zero.
    case "$ASKED" in
        *"stored=8"*) fail "SELFTEST=1: the requester reached stored=8 and the run must end non-zero: $ASKED" ;;
        *) ;;
    esac
else
    case "$ASKED" in
        *"stored=8"*) ;;
        *) fail "the requester did not bring the filtered item to max 8: $ASKED" ;;
    esac
    case "$ASKED" in
        *"phase=idle"*) ;;
        *) fail "the requester did not stop at its maximum: $ASKED" ;;
    esac
    case "$ASKCRAFT" in
        *"stored=8"*"crafted=8"*) ;;
        *) fail "the crafter did not make 8 torches on the requester's ask: $ASKCRAFT" ;;
    esac
    case "$ASKCRAFT" in
        *"minecraft:coal:2"*"minecraft:stick:2"*) ;;
        *) fail "the two asked-for steps did not consume exactly 2 coal and 2 sticks: $ASKCRAFT" ;;
    esac
fi

echo "== the same world with the requester removed leaves a crafter at zero =="
ctl command "execute in minecraft:the_end run setblock 212 64 0 minecraft:air"
ctl wait 6
ctl command "execute in minecraft:the_end run setblock 213 64 0 $BLOCK"
ctl wait 6
say "stepcrafter target 213 64 0 minecraft:torch"
reply '^stepcrafter target 213 64 0 ' >/dev/null
say "stepcrafter configure 213 64 0 0 0"
reply '^stepcrafter configure 213 64 0 ' >/dev/null
load 213 64 0 minecraft:coal 4
load 213 64 0 minecraft:stick 4
ctl wait 40
ALONE=$(info 213 64 0)
echo "  a crafter with no requester, same world: $ALONE"
case "$ALONE" in
    *"stored=0"*"crafted=0"*) ;;
    *) fail "a crafter with no requester crafted on its own: $ALONE" ;;
esac

echo "== a crafter that cannot step: the requester reports it, and the restock resumes it =="
say "stepcrafter take 213 64 0 minecraft:stick 4"
reply '^stepcrafter take 213 64 0 ' >/dev/null
ctl command "execute in minecraft:the_end run setblock 215 64 0 $REQUESTER"
ctl wait 6
say "stepcrafter requester 215 64 0 minecraft:torch 4 8 4"
reply '^stepcrafter requester 215 64 0 ' >/dev/null
ctl wait 40
REFUSED=$(info 215 64 0)
REFUSED_CRAFT=$(info 213 64 0)
echo "  requester: $REFUSED"
echo "  crafter:   $REFUSED_CRAFT"
case "$REFUSED" in
    *"phase=not-enough-ingredients"*) ;;
    *) fail "the requester did not report the crafter's refusal: $REFUSED" ;;
esac
case "$REFUSED_CRAFT" in
    *"crafted=0"*) ;;
    *) fail "the crafter crafted without its sticks: $REFUSED_CRAFT" ;;
esac
load 213 64 0 minecraft:stick 4
ctl wait 140
RESUMED=$(info 215 64 0)
RESUMED_CRAFT=$(info 213 64 0)
echo "  restocked: requester $RESUMED"
echo "  restocked: crafter   $RESUMED_CRAFT"
case "$RESUMED" in
    *"stored=8"*"phase=idle"*) ;;
    *) fail "the requester did not resume after the restock: $RESUMED" ;;
esac
case "$RESUMED_CRAFT" in
    *"crafted=8"*) ;;
    *) fail "the restocked crafter did not resume its steps: $RESUMED_CRAFT" ;;
esac

echo "== the requester's active state follows its run =="
# A second pair of its own, out of the first pair's reach, for a run long enough (32 steps of 4
# torches to 128) that `running` and `active` are readable while it works.
ctl command "execute in minecraft:the_end run setblock 197 64 4 $BLOCK"
ctl wait 6
say "stepcrafter target 197 64 4 minecraft:torch"
reply '^stepcrafter target 197 64 4 ' >/dev/null
say "stepcrafter configure 197 64 4 0 0"
reply '^stepcrafter configure 197 64 4 ' >/dev/null
load 197 64 4 minecraft:coal 32
load 197 64 4 minecraft:stick 32
ctl command "execute in minecraft:the_end run setblock 196 64 4 $REQUESTER"
ctl wait 6
say "stepcrafter requester 196 64 4 minecraft:torch 4 128 4"
reply '^stepcrafter requester 196 64 4 ' >/dev/null
# 32 steps is a 32-tick window: the read lands inside it, so `phase=running` is the real phase.
ctl wait 2
RUNAWAY=$(info 196 64 4)
echo "  while it runs: $RUNAWAY"
case "$RUNAWAY" in
    *"phase=running"*) ;;
    *) fail "the requester did not report a running ask: $RUNAWAY" ;;
esac
RUNNING=$(active_is 196 64 4 true "$REQUESTER")
echo "  active=true while asking: $RUNNING"
case "$RUNNING" in
    *"Test passed"*) ;;
    *) fail "the requester did not show active=true while asking: $RUNNING" ;;
esac
ctl wait 400
DONE=$(active_is 196 64 4 false "$REQUESTER")
echo "  active=false once done: $DONE"
case "$DONE" in
    *"Test passed"*) ;;
    *) fail "the requester still shows active=true once idle: $DONE" ;;
esac
expect 196 64 4 "stored=128 served=128 queued=0 phase=idle"
expect 197 64 4 "stored=128"
FILLED_CRAFT=$(info 197 64 4)
case "$FILLED_CRAFT" in
    *"crafted=128"*) ;;
    *) fail "the long run did not craft exactly 128 torches: $FILLED_CRAFT" ;;
esac

echo "== a result that fits no slot is refused: nothing consumed, nothing served =="
# The review's defect: the machine's own band never fires (0..0) and its 27 slots are 26 stacks of
# 64 coal plus 60 sticks — 1,724 items, so an aggregate budget of 1,728 says four torches fit. No
# slot is empty and none holds a torch, so the result has nowhere to go: every ask the requester
# beside it makes must be refused, consuming nothing and recording nothing as crafted.
ctl command "execute in minecraft:the_end run setblock 216 64 0 $BLOCK"
ctl wait 6
say "stepcrafter target 216 64 0 minecraft:torch"
reply '^stepcrafter target 216 64 0 ' >/dev/null
say "stepcrafter configure 216 64 0 0 0"
reply '^stepcrafter configure 216 64 0 ' >/dev/null
for _ in $(seq 1 26); do load 216 64 0 minecraft:coal 64; done
load 216 64 0 minecraft:stick 60
FULL=$(info 216 64 0)
echo "  the full-ish machine: $FULL"
case "$FULL" in
    *"stock=minecraft:coal:1664,minecraft:stick:60"*) ;;
    *) fail "the machine was not stocked 26x64 coal + 60 sticks: $FULL" ;;
esac
ctl command "execute in minecraft:the_end run setblock 217 64 0 $REQUESTER"
ctl wait 6
say "stepcrafter requester 217 64 0 minecraft:torch 4 8 4"
reply '^stepcrafter requester 217 64 0 ' >/dev/null
ctl wait 60
NO_ROOM=$(info 216 64 0)
NO_ROOM_REQ=$(info 217 64 0)
echo "  after the asks, the machine:   $NO_ROOM"
echo "  after the asks, the requester: $NO_ROOM_REQ"
case "$NO_ROOM" in
    *"stored=0"*"phase=network-full"*"crafted=0"*) ;;
    *) fail "a step that fits no slot was not refused as network-full with nothing crafted: $NO_ROOM" ;;
esac
case "$NO_ROOM" in
    *"minecraft:coal:1664"*"minecraft:stick:60"*) ;;
    *) fail "the refused step consumed ingredients: $NO_ROOM" ;;
esac
case "$NO_ROOM_REQ" in
    *"kind=requester"*"stored=0"*) ;;
    *) fail "the requester did not read the refusal with nothing stored: $NO_ROOM_REQ" ;;
esac
if [ "$SELFTEST" = capacity ]; then
    # The fixed behaviour, deliberately inverted: nothing fit and nothing was served, so a run that
    # must be able to fail has to end here, non-zero.
    case "$NO_ROOM" in
        *"crafted=0"*) fail "SELFTEST=capacity: the step fit no slot and served nothing, and the run must end non-zero: $NO_ROOM" ;;
        *) ;;
    esac
fi

echo "== a named stack of the target is no room for the step's plain result =="
# The last silent-loss path (notes.md §6): the room model counted a stack of the result by item id,
# so 60 named torches offered the 4 items of room their count left, while the insert merges only into
# a stack with no components. The step counted four torches as crafted and stored none. The machine
# here holds that named stack and the recipe's ingredients in every other slot (26 x 64 + 60 =
# 1,724 items, none empty), so the only candidate for the result is the named stack: it must refuse.
ctl command "execute in minecraft:the_end run setblock 221 64 0 $BLOCK"
ctl wait 6
say "stepcrafter target 221 64 0 minecraft:torch"
reply '^stepcrafter target 221 64 0 ' >/dev/null
say "stepcrafter configure 221 64 0 0 0"
reply '^stepcrafter configure 221 64 0 ' >/dev/null
plant_room 221 64 0 named
NAMED_BEFORE=$(info 221 64 0)
echo "  before the step: $NAMED_BEFORE"
case "$NAMED_BEFORE" in
    *"stored=60"*"items=1724"*"stock=minecraft:coal:1600,minecraft:stick:64,minecraft:torch:60"*) ;;
    *) fail "the named torch stack and the full inventory were not planted: $NAMED_BEFORE" ;;
esac
say "stepcrafter step 221 64 0"
NAMED_STEP=$(reply '^stepcrafter step 221 64 0 ')
echo "  $NAMED_STEP"
case "$NAMED_STEP" in
    *"result=network-full"*) ;;
    *) fail "a step whose result only fits a named stack was not refused: $NAMED_STEP" ;;
esac
NAMED=$(info 221 64 0)
echo "  after the step: $NAMED"
case "$NAMED" in
    *"stored=60"*"phase=network-full"*"crafted=0"*) ;;
    *) fail "the named stack counted room the game would not use: $NAMED" ;;
esac
case "$NAMED" in
    *"stock=minecraft:coal:1600,minecraft:stick:64,minecraft:torch:60"*) ;;
    *) fail "the refused step consumed ingredients: $NAMED" ;;
esac
if [ "$SELFTEST" = components-room ] || [ "$SELFTEST" = components ]; then
    # The defect, deliberately asserted: today the step counted four torches it never stored beside
    # the unchanged named stack. The package refuses it, so this inverted run must end non-zero.
    case "$NAMED" in
        *"phase=network-full"*"crafted=0"*)
            fail "SELFTEST=components-room: the named stack counted no room and the step was refused — the fixed behaviour — and the run must end non-zero: $NAMED" ;;
        *) ;;
    esac
fi

echo "== the same step beside a plain stack merges its four torches (the differential) =="
# Only the component patch differs from the machine above: a plain stack of 60 torches has the same
# 4 items of nominal room, and this time the merge rule uses it.
ctl command "execute in minecraft:the_end run setblock 222 64 0 $BLOCK"
ctl wait 6
say "stepcrafter target 222 64 0 minecraft:torch"
reply '^stepcrafter target 222 64 0 ' >/dev/null
say "stepcrafter configure 222 64 0 0 0"
reply '^stepcrafter configure 222 64 0 ' >/dev/null
plant_room 222 64 0 plain
PLAIN_BEFORE=$(info 222 64 0)
echo "  before the step: $PLAIN_BEFORE"
case "$PLAIN_BEFORE" in
    *"stored=60"*"items=1724"*"stock=minecraft:coal:1600,minecraft:stick:64,minecraft:torch:60"*) ;;
    *) fail "the plain torch stack and the full inventory were not planted: $PLAIN_BEFORE" ;;
esac
say "stepcrafter step 222 64 0"
PLAIN_STEP=$(reply '^stepcrafter step 222 64 0 ')
echo "  $PLAIN_STEP"
case "$PLAIN_STEP" in
    *"result=crafted"*) ;;
    *) fail "the same step beside a plain stack was refused: $PLAIN_STEP" ;;
esac
PLAIN=$(info 222 64 0)
echo "  after the step: $PLAIN"
case "$PLAIN" in
    *"stored=64"*"crafted=4"*"stock=minecraft:coal:1599,minecraft:stick:63,minecraft:torch:64"*) ;;
    *) fail "the four torches did not merge into the plain stack: $PLAIN" ;;
esac

echo "== the Step Requester keeps several filter entries, each served by its crafter =="
# Upstream's requester has 45 filter slots (StepRequesterBlockEntity.FILTERS = 9 * 5), each a
# resource with its own band, batch and status; 0.2.4 kept one. One requester here carries two
# entries and reaches two crafters, each maintaining the entry's own item. The component-room
# machines above are cleared first, so this leg's requester and crafters own 221/222/223.
for pos in 221 222 223; do
    ctl command "execute in minecraft:the_end run setblock $pos 64 0 minecraft:air" >/dev/null
done
ctl wait 6
ctl command "execute in minecraft:the_end run setblock 221 64 0 $BLOCK"
ctl command "execute in minecraft:the_end run setblock 223 64 0 $BLOCK"
ctl command "execute in minecraft:the_end run setblock 222 64 0 $REQUESTER"
ctl wait 8
say "stepcrafter target 221 64 0 minecraft:torch"
reply '^stepcrafter target 221 64 0 ' >/dev/null
say "stepcrafter configure 221 64 0 0 0"
reply '^stepcrafter configure 221 64 0 ' >/dev/null
say "stepcrafter target 223 64 0 minecraft:stick"
reply '^stepcrafter target 223 64 0 ' >/dev/null
say "stepcrafter configure 223 64 0 0 0"
reply '^stepcrafter configure 223 64 0 ' >/dev/null
load 221 64 0 minecraft:coal 4
load 221 64 0 minecraft:stick 4
# `minecraft:planks` is a tag, not an item: the stock has to name a real plank (`ctl container`
# silently stocks nothing for a tag, which is exactly the trap this assertion closes).
load 223 64 0 minecraft:oak_planks 8
MULTI_STOCK=$(info 223 64 0)
echo "  the stick crafter before the filters: $MULTI_STOCK"
case "$MULTI_STOCK" in
    *"stock=minecraft:oak_planks:8"*) ;;
    *) fail "the stick crafter was not stocked with 8 planks: $MULTI_STOCK" ;;
esac
say "stepcrafter requester add 222 64 0 minecraft:torch 4 8 4"
MULTI_FIRST=$(reply '^stepcrafter requester 222 64 0 ')
echo "  the first entry: $MULTI_FIRST"
case "$MULTI_FIRST" in
    *"item=minecraft:torch min=4 max=8 batch=4 filters=1"*) ;;
    *) fail "the first filter entry was not added: $MULTI_FIRST" ;;
esac
say "stepcrafter requester add 222 64 0 minecraft:stick 8 16 8"
MULTI_SECOND=$(reply '^stepcrafter requester 222 64 0 ')
echo "  the second entry: $MULTI_SECOND"
case "$MULTI_SECOND" in
    *"item=minecraft:stick min=8 max=16 batch=8 filters=2"*) ;;
    *) fail "the second filter entry was not added beside the first: $MULTI_SECOND" ;;
esac
ctl wait 80
MULTI=$(info 222 64 0)
MULTI_TORCH=$(info 221 64 0)
MULTI_STICK=$(info 223 64 0)
echo "  the requester:      $MULTI"
echo "  the torch crafter:  $MULTI_TORCH"
echo "  the stick crafter:  $MULTI_STICK"
if [ "$SELFTEST" = filters ]; then
    # The defect this version closes, deliberately asserted: with one entry the requester would read
    # `filters=1` and only the newest resource would be served. The package keeps both, so this
    # inverted run has to end here, non-zero.
    case "$MULTI" in
        *"filters=1"*) ;;
        *) fail "SELFTEST=filters: the requester keeps both filter entries (filters=2) where today's defect keeps one (filters=1), so the run must end non-zero: $MULTI" ;;
    esac
else
    case "$MULTI" in
        *"filters=2"*) ;;
        *) fail "the requester did not keep both filter entries: $MULTI" ;;
    esac
    # Both entries, with their own bands, from the readout's per-entry list (the fields after the
    # batch are the entry's stored count, share, queue flag, phase, crafter and timeout).
    case "$MULTI" in
        *"entries=minecraft:torch|4|8|4|"*"minecraft:stick|8|16|8|"*) ;;
        *) fail "the two entries were not both served by their own crafter: $MULTI" ;;
    esac
    case "$MULTI_TORCH" in
        *"crafted=8"*) ;;
        *) fail "the torch entry's ask did not make 8 torches: $MULTI_TORCH" ;;
    esac
    case "$MULTI_STICK" in
        *"crafted=16"*) ;;
        *) fail "the stick entry's ask did not make 16 sticks: $MULTI_STICK" ;;
    esac
fi

echo "== the same world with one entry removed leaves the stick crafter at zero =="
# A second requester with only the torch entry, beside its own two crafters (the same items and
# stocks). The stick crafter is configured and stocked exactly like the one above and stays at
# crafted=0 because no entry asks for sticks: the removed entry is the only difference.
for pos in 221 222 223; do
    ctl command "execute in minecraft:the_end run setblock $pos 64 4 minecraft:air" >/dev/null
done
ctl wait 6
ctl command "execute in minecraft:the_end run setblock 221 64 4 $BLOCK"
ctl command "execute in minecraft:the_end run setblock 223 64 4 $BLOCK"
ctl command "execute in minecraft:the_end run setblock 222 64 4 $REQUESTER"
ctl wait 8
say "stepcrafter target 221 64 4 minecraft:torch"
reply '^stepcrafter target 221 64 4 ' >/dev/null
say "stepcrafter configure 221 64 4 0 0"
reply '^stepcrafter configure 221 64 4 ' >/dev/null
say "stepcrafter target 223 64 4 minecraft:stick"
reply '^stepcrafter target 223 64 4 ' >/dev/null
say "stepcrafter configure 223 64 4 0 0"
reply '^stepcrafter configure 223 64 4 ' >/dev/null
load 221 64 4 minecraft:coal 4
load 221 64 4 minecraft:stick 4
load 223 64 4 minecraft:oak_planks 8
# The 0.2.1–0.2.4 spelling sets (or replaces) one entry, so this requester carries a single one.
say "stepcrafter requester 222 64 4 minecraft:torch 4 8 4"
ONE=$(reply '^stepcrafter requester 222 64 4 ')
echo "  the one-entry requester: $ONE"
case "$ONE" in
    *"item=minecraft:torch min=4 max=8 batch=4 filters=1"*) ;;
    *) fail "the one-entry requester was not configured: $ONE" ;;
esac
ctl wait 80
ONE_REQ=$(info 222 64 4)
ONE_TORCH=$(info 221 64 4)
ONE_STICK=$(info 223 64 4)
echo "  the requester:     $ONE_REQ"
echo "  the torch crafter: $ONE_TORCH"
echo "  the stick crafter: $ONE_STICK"
case "$ONE_REQ" in
    *"filters=1"*) ;;
    *) fail "the one-entry requester did not read one entry: $ONE_REQ" ;;
esac
case "$ONE_TORCH" in
    *"crafted=8"*) ;;
    *) fail "the one entry did not serve its own torch crafter: $ONE_TORCH" ;;
esac
case "$ONE_STICK" in
    *"crafted=0"*) ;;
    *) fail "a crafter was asked without a filter entry for its item: $ONE_STICK" ;;
esac

echo "== take hands over the stored stacks: components and damage survive =="
ctl command "execute in minecraft:the_end run setblock 218 64 0 $BLOCK"
ctl wait 6
# An empty inventory, so the named stacks have room when they are handed over.
ctl command "clear"
ctl wait 4
# Two differently named torch stacks and one damaged, enchanted sword, planted in the machine's
# container through the pin's block-entity write path (the same `/data merge block` the requester's
# ask uses). A sword stacks to one, so its count is one.
ctl command 'data merge block 218 64 0 {Items:[{Slot:0b,id:"minecraft:torch",count:2,components:{"minecraft:custom_name":"Alpha"}},{Slot:1b,id:"minecraft:torch",count:3,components:{"minecraft:custom_name":"Beta"}},{Slot:2b,id:"minecraft:diamond_sword",count:1,components:{"minecraft:custom_name":"Blade","minecraft:damage":7,"minecraft:enchantments":{"minecraft:sharpness":3}}}]}' >/dev/null
ctl wait 6
PLANTED=$(info 218 64 0)
echo "  the planted stacks: $PLANTED"
case "$PLANTED" in
    *"stored=0"*"items=6"*"minecraft:diamond_sword:1"*"minecraft:torch:5"*) ;;
    *) fail "the component-carrying stacks were not planted: $PLANTED" ;;
esac
say "stepcrafter take 218 64 0 minecraft:torch 5"
TOOK=$(reply '^stepcrafter take 218 64 0 ')
echo "  $TOOK"
case "$TOOK" in
    *"item=minecraft:torch stored=0 taken=5"*) ;;
    *) fail "take did not hand over the five torches: $TOOK" ;;
esac
say "stepcrafter take 218 64 0 minecraft:diamond_sword 1"
TOOK_SWORD=$(reply '^stepcrafter take 218 64 0 ')
echo "  $TOOK_SWORD"
case "$TOOK_SWORD" in
    *"item=minecraft:diamond_sword stored=0 taken=1"*) ;;
    *) fail "take did not hand over the sword: $TOOK_SWORD" ;;
esac
ctl wait 6
TORCHES=$(ctl inventory | jq -c '[.hotbar[], .main[] | select(.item == "minecraft:torch") | {count, components}]')
TORCH_COMPONENTS=$(jq -r 'map(.components // "") | join(" ")' <<<"$TORCHES")
SWORD=$(ctl inventory | jq -c '[.hotbar[], .main[] | select(.item == "minecraft:diamond_sword")][0]')
SWORD_COMPONENTS=$(jq -r '.components // ""' <<<"$SWORD")
SWORD_DAMAGE=$(jq -r '.damage // 0' <<<"$SWORD")
echo "  torches now in the inventory: $TORCHES"
echo "  the sword now in the inventory: $SWORD"
case "$TORCH_COMPONENTS" in
    *"Alpha"*) ;;
    *) fail "the taken stack named Alpha lost its components: $TORCHES" ;;
esac
case "$TORCH_COMPONENTS" in
    *"Beta"*) ;;
    *) fail "the taken stack named Beta lost its components: $TORCHES" ;;
esac
jq -e 'map(select(.count == 2)) | length == 1' >/dev/null <<<"$TORCHES" \
    || fail "the two Alpha torches were not handed over as their own stack: $TORCHES"
jq -e 'map(select(.count == 3)) | length == 1' >/dev/null <<<"$TORCHES" \
    || fail "the three Beta torches were not handed over as their own stack: $TORCHES"
case "$SWORD_COMPONENTS" in
    *"Blade"*) ;;
    *) fail "the taken sword lost its name: $SWORD" ;;
esac
case "$SWORD_COMPONENTS" in
    *"sharpness"*) ;;
    *) fail "the taken sword lost its enchantment: $SWORD" ;;
esac
[ "$SWORD_DAMAGE" = 7 ] || fail "the taken sword lost its damage 7: $SWORD"
if [ "$SELFTEST" = take ]; then
    # The defect, deliberately asserted: a stack rebuilt from its item id carries no components and
    # no damage. The package keeps them, so this inverted run has to end here, non-zero.
    case "$TORCH_COMPONENTS$SWORD_COMPONENTS" in
        "") ;;
        *) fail "SELFTEST=take: the taken stacks kept their components and the run must end non-zero: $TORCHES $SWORD" ;;
    esac
fi

echo "== crafting remainders: a step leaves the bucket the recipe leaves =="
# The gap this version closes (notes.md §6/§10): a bucket ingredient used to be destroyed, where a
# real crafting grid hands its bucket back. A cake takes three milk buckets; one step must leave the
# cake *and* three buckets, and consume the sugar, egg and wheat.
ctl command "execute in minecraft:the_end run setblock 220 64 0 $BLOCK"
ctl wait 6
say "stepcrafter target 220 64 0 minecraft:cake"
CAKE_TARGET=$(reply '^stepcrafter target 220 64 0 ')
echo "  $CAKE_TARGET"
case "$CAKE_TARGET" in
    *"target=minecraft:cake cells=9 result=minecraft:cake:1"*) ;;
    *) fail "the cake target did not resolve to a 9-cell step: $CAKE_TARGET" ;;
esac
# The step pattern carries what its ingredients leave: the milk bucket's own remainder.
case "$CAKE_TARGET" in
    *"remainders=minecraft:milk_bucket=minecraft:bucket:1"*) ;;
    *) fail "the cake pattern does not carry the bucket its milk leaves: $CAKE_TARGET" ;;
esac
say "stepcrafter configure 220 64 0 0 0"
reply '^stepcrafter configure 220 64 0 ' >/dev/null
# A milk bucket stacks to one, so three of them go in three slots.
load 220 64 0 minecraft:milk_bucket 1
load 220 64 0 minecraft:milk_bucket 1
load 220 64 0 minecraft:milk_bucket 1
load 220 64 0 minecraft:sugar 2
load 220 64 0 minecraft:egg 1
load 220 64 0 minecraft:wheat 3
CAKE_BEFORE=$(info 220 64 0)
echo "  before the step: $CAKE_BEFORE"
case "$CAKE_BEFORE" in
    *"stock=minecraft:egg:1,minecraft:milk_bucket:3,minecraft:sugar:2,minecraft:wheat:3"*) ;;
    *) fail "the machine was not stocked with cake's ingredients: $CAKE_BEFORE" ;;
esac
say "stepcrafter step 220 64 0"
CAKE_STEP=$(reply '^stepcrafter step 220 64 0 ')
echo "  $CAKE_STEP"
case "$CAKE_STEP" in
    *"result=crafted"*) ;;
    *) fail "the cake step was refused: $CAKE_STEP" ;;
esac
ctl wait 6
CAKE_AFTER=$(info 220 64 0)
echo "  after the step: $CAKE_AFTER"
case "$CAKE_AFTER" in
    *"stored=1"*"crafted=1"*) ;;
    *) fail "the cake step did not make the cake: $CAKE_AFTER" ;;
esac
# The output *and* the three buckets, with every ingredient consumed: the exact stock, not a grep
# that would also match the ingredients.
case "$CAKE_AFTER" in
    *"stock=minecraft:bucket:3,minecraft:cake:1"*) ;;
    *) fail "the step did not leave the three buckets its milk leaves: $CAKE_AFTER" ;;
esac
if [ "$SELFTEST" = remainder ]; then
    # Today's defect, deliberately asserted: the step consumes every cell and puts only the result
    # in, so the buckets are destroyed. The package leaves them, so this inverted run has to end
    # here, non-zero.
    case "$CAKE_AFTER" in
        *"minecraft:bucket"*) fail "SELFTEST=remainder: the step left the buckets its milk leaves and the run must end non-zero: $CAKE_AFTER" ;;
        *) ;;
    esac
fi

echo "== /stepcrafter requester refuses what it cannot configure =="
say "stepcrafter requester 215 64 0 minecraft:torch 9 4 4"
INVERTED_FILTER=$(reply '^stepcrafter: min 9 is above max 4')
echo "  $INVERTED_FILTER"
case "$INVERTED_FILTER" in
    *"min 9 is above max 4"*) ;;
    *) fail "an inverted requester band was not refused: $INVERTED_FILTER" ;;
esac
say "stepcrafter requester 215 64 0 minecraft:not_a_thing 4 8 4"
UNKNOWN_FILTER=$(reply '^stepcrafter: minecraft:not_a_thing')
echo "  $UNKNOWN_FILTER"
case "$UNKNOWN_FILTER" in
    *"is not an item this world knows"*) ;;
    *) fail "an unknown filter item was not refused: $UNKNOWN_FILTER" ;;
esac
say "stepcrafter requester 215 64 0 minecraft:torch 4 8 -1"
NEGATIVE_FILTER=$(reply '^stepcrafter: min, max and batch')
echo "  $NEGATIVE_FILTER"
case "$NEGATIVE_FILTER" in
    *"not negative numbers"*) ;;
    *) fail "a negative batch was not refused: $NEGATIVE_FILTER" ;;
esac
say "stepcrafter target 215 64 0 minecraft:torch"
WRONG_BLOCK=$(reply '^stepcrafter: that is a Step Requester')
echo "  $WRONG_BLOCK"
case "$WRONG_BLOCK" in
    *"that is a Step Requester"*) ;;
    *) fail "a crafter command on a requester was not refused: $WRONG_BLOCK" ;;
esac

echo "== the requester's own loot =="
ctl command "gamemode survival @s"
ctl wait 6
ctl give minecraft:diamond_pickaxe 1 >/dev/null
ctl wait 4
ctl select minecraft:diamond_pickaxe >/dev/null
ctl wait 3
ctl command "execute in minecraft:the_end run tp @s 215.5 64 3.5"
ctl wait 6
BROKE_REQ=$(ctl break 215 64 0) || fail "cannot break the requester"
ctl wait 10
REQ_GONE=$(ctl block 215 64 0 | jq -r '.block')
REQ_DROPS=$(jq -r '.drops | to_entries | map("\(.key):\(.value)") | join(",")' <<<"$BROKE_REQ" 2>/dev/null || true)
echo "  requester after the break: $REQ_GONE; drops: $REQ_DROPS"
[ "$REQ_GONE" = "minecraft:air" ] || fail "the requester did not break: $REQ_GONE"
case "$REQ_DROPS" in
    *"$REQUESTER"*) ;;
    *) fail "the broken requester dropped no item of its own: $REQ_DROPS" ;;
esac
ctl command "gamemode creative"
ctl wait 4

echo "== a broken machine carries its configuration and spills its stock =="
ctl command "execute in minecraft:the_end run setblock 214 64 0 $BLOCK"
ctl wait 6
say "stepcrafter target 214 64 0 minecraft:torch"
reply '^stepcrafter target 214 64 0 ' >/dev/null
load 214 64 0 minecraft:coal 2
load 214 64 0 minecraft:stick 2
say "stepcrafter configure 214 64 0 4 8"
reply '^stepcrafter configure 214 64 0 ' >/dev/null
ctl wait 40
CARRIED=$(info 214 64 0)
echo "  before the break: $CARRIED"
case "$CARRIED" in
    *"stored=8"*) ;;
    *) fail "the machine did not fill to 8 before the break: $CARRIED" ;;
esac
ctl command "execute in minecraft:the_end run tp @s 214.5 64 3.5"
ctl wait 6
# A creative break drops nothing at all, the container's contents included, so the break is made
# by a survival player (the way a player breaks a machine) and the creative mode comes back after.
ctl command "gamemode survival @s" >/dev/null 2>&1 || ctl command "gamemode survival" >/dev/null
ctl wait 6
# The block needs a pickaxe (`requires_tool`, as upstream's stone-like block does): a bare hand
# breaks it and drops nothing but the spill.
ctl give minecraft:diamond_pickaxe 1 >/dev/null
ctl wait 4
ctl select minecraft:diamond_pickaxe >/dev/null
ctl wait 3
echo "  game mode now: $(ctl state | jq -r '.creative'), chat: $(ctl messages 4 | jq -c '.chat')"
BROKEN=$(ctl break 214 64 0) || fail "cannot break the machine"
ctl wait 20
GONE=$(ctl block 214 64 0 | jq -r '.block')
echo "  break reply: $(jq -c '{block, drops}' <<<"$BROKEN" 2>/dev/null || echo "$BROKEN")"
echo "  block(214,64,0) after the break: $GONE"
[ "$GONE" = "minecraft:air" ] || fail "the machine did not break: $GONE"
# The stock is the inventory, so the engine spills it: the break's own reply names what dropped,
# the 8 torches among it, beside the block's item from its loot table.
DROPPED=$(jq -r '.drops | to_entries | map("\(.key):\(.value)") | join(",")' <<<"$BROKEN" 2>/dev/null || true)
echo "  drops: $DROPPED"
case "$DROPPED" in
    *"minecraft:torch"*) ;;
    *) fail "the broken machine spilled no torches: $DROPPED" ;;
esac
case "$DROPPED" in
    *"$BLOCK"*) ;;
    *) fail "the broken machine dropped no item of its own: $DROPPED" ;;
esac
ctl command "gamemode creative"
ctl wait 4
# Pick the drops up (the player stands beside them) and place the machine again.
ctl command "execute in minecraft:the_end run tp @s 214.5 64 0.5"
ctl wait 30
HELD=$(inventory_count "$BLOCK")
SPILLED=$(inventory_count minecraft:torch)
echo "  Step Crafter items held: $HELD; torches picked up: $SPILLED"
[ "$HELD" -ge 1 ] || fail "the broken machine's item was not picked up"
[ "$SPILLED" -ge 8 ] || fail "the spilled torches did not reach the inventory: $SPILLED"
ctl command "item replace entity @s hotbar.0 with $BLOCK 1" >/dev/null
ctl wait 4
ctl select 1 >/dev/null
ctl wait 3
# Step back off the block space: a player standing in it cannot place the machine there.
ctl command "execute in minecraft:the_end run tp @s 214.5 64 3.5" >/dev/null
ctl wait 10
# The break leaves the spilled stack as item entities; any that are still around the spot block
# the placement ("blocked by an entity"), as does an Enderman that wandered onto the spot (the End
# spawns them, and a run is minutes long). The drops were picked up above and `held`/`SPILLED`
# already asserted that, so clear whatever is left on the spot — items and mobs alike.
ctl command "execute in minecraft:the_end run kill @e[type=!player,x=213,y=63,z=-1,dx=3,dy=4,dz=3]" >/dev/null 2>&1 || true
ctl wait 4
# `--item` selects the hotbar slot holding the machine by id, not by a remembered slot number; the
# second attempt covers a teleport that lands after the first click's aim.
PLACED=$(ctl place 214 64 0 --item "$BLOCK" 2>&1) \
    || PLACED=$(ctl place 214 64 0 --item "$BLOCK" 2>&1) \
    || fail "cannot place the machine again: $PLACED"
ctl wait 10
REPLACED=$(info 214 64 0)
echo "  after placing it back: $REPLACED"
# The configuration does not travel, and that is this port's measured limit rather than a silent
# loss: upstream's pattern is an item that drops, while this port's pattern is derived from the
# target id, and a container block's contents spill (HANDOVER §9d.23 forbids a `block_entity_data`
# payload that would double-carry them). A moved machine is re-targeted.
case "$REPLACED" in
    *"target=none min=- max=- stored=0"*) ;;
    *) fail "a moved machine should read unconfigured (the recorded limit): $REPLACED" ;;
esac

echo "== the requester's filter screen: the same saved container as the command =="
# The slice after 0.2.5 (notes.md §1/§6): the 45 filter entries upstream edits in its GUI. The
# screen's cells are drawn (a resource, a band and a batch are not an item stack), and a press is a
# menu button: the client hit-tests it (`screen_click`), the server applies it (`menu_button`).
# The multi-entry leg's machines own 221..223; that leg is done, so the brief's cell is reused for
# the screen and the differential keeps a requester beside it that nobody opens.
for pos in 221 222 223; do
    for z in 0 4; do
        ctl command "execute in minecraft:the_end run setblock $pos 64 $z minecraft:air" >/dev/null
    done
done
ctl wait 6
ctl command "execute in minecraft:the_end run setblock 223 64 0 $REQUESTER"
ctl command "execute in minecraft:the_end run setblock 223 64 4 $REQUESTER"
ctl command "execute in minecraft:the_end run setblock 222 64 0 $BLOCK"
ctl wait 8

# A mod panel's origin, read back from the game: `slot-0` is the hotbar's first slot (vanilla's
# inventory order), which the screen declares at (8, inventory_y + 58), so its absolute bounds invert
# to the panel's top-left. Both answers must be the panel centred in the reported GUI.
panel_origin() { # INVENTORY_Y
    local ui rect ox oy w h
    ui=$(ctl ui)
    rect=$(jq -c '[.widgets[] | select(.id == "slot-0") | .bounds][0]' <<<"$ui")
    [ "$rect" != null ] || { echo "$ui" >&2; fail "the mod screen shows no player-inventory slot (slot-0)"; }
    ox=$(jq -r '.[0] | floor' <<<"$rect")
    oy=$(jq -r '.[1] | floor' <<<"$rect")
    OX=$(( ox - 8 ))
    OY=$(( oy - $1 - 58 ))
    w=$(jq -r '.gui_size[0] | floor' <<<"$ui")
    h=$(jq -r '.gui_size[1] | floor' <<<"$ui")
    [ "$OX" = "$(( (w - 176) / 2 ))" ] || fail "the panel's left is $OX, not the centred 176-wide panel"
    [ "$OY" = "$(( (h - PANEL_H) / 2 ))" ] || fail "the panel's top is $OY, not the centred $PANEL_H-high panel"
}

open_requester() { # X Y Z -> sets OPENED (the screen's name) and, when it opened, OX/OY
    ctl close >/dev/null 2>&1 || true
    ctl tp "$1.5" "$2" "$(( $3 + 2 )).5" >/dev/null
    ctl wait 6
    ctl look 180 0 >/dev/null
    ctl wait 2
    ctl look-at block "$1" "$2" "$3" south >/dev/null 2>&1 || true
    ctl interact "$1" "$2" "$3" south >/dev/null || fail "the requester refused the right click"
    ctl wait 6
    OPENED=$(ctl screen | jq -r '.screen')
    if [ "$OPENED" = container ]; then PANEL_H=238; panel_origin 156; fi
}

press() { # LOCAL_X LOCAL_Y: a press at a point of the panel
    ctl click-at $(( OX + $1 )) $(( OY + $2 )) >/dev/null || fail "the press at $1,$2 did not send"
    ctl wait 2
}

# The drawn cells and buttons, at the pixels `src/screen.rs` lays them out at (a cell's centre, a
# value button's centre, the remove button's).
press_cell() { press $(( 8 + ($1 % 9) * 18 + 9 )) $(( 18 + ($1 / 9) * 18 + 9 )); }
press_min_down() { press 32 116; }
press_min_up() { press 72 116; }
press_max_down() { press 102 116; }
press_max_up() { press 142 116; }
press_batch_down() { press 44 134; }
press_batch_up() { press 84 134; }
press_remove() { press 130 134; }

# The machine's own panel opens on the same click path (the engine never opens a mod container's
# menu): since 0.2.8 the machine opens the package's own menu type, whose 27 stock slots are still
# real menu slots, and whose drawn cell and buttons sit under them.
ctl close >/dev/null 2>&1 || true
ctl tp 222.5 64 2.5 >/dev/null
ctl wait 6
ctl look 180 0 >/dev/null
ctl wait 2
ctl look-at block 222 64 0 south >/dev/null 2>&1 || true
ctl interact 222 64 0 south >/dev/null || fail "the machine refused the right click"
ctl wait 6
MACHINE_SCREEN=$(ctl screen)
[ "$(jq -r '.screen' <<<"$MACHINE_SCREEN")" = container ] \
    || fail "the machine's menu did not open: $(jq -r '.screen' <<<"$MACHINE_SCREEN")"
MACHINE_SLOTS=$(jq '[.slots[] | select(.slot >= 300)] | length' <<<"$MACHINE_SCREEN")
PANEL_H=208
panel_origin 126
MACHINE_OY=$OY
echo "  the machine's stock menu: $(jq -r '.screen' <<<"$MACHINE_SCREEN"), $MACHINE_SLOTS container slots, top $MACHINE_OY"
[ "$MACHINE_SLOTS" = 27 ] || fail "the machine's menu shows $MACHINE_SLOTS slots, not its 27"
ctl close >/dev/null
ctl wait 4

ctl command "clear" >/dev/null
ctl wait 4
ctl give minecraft:torch 1 >/dev/null
ctl wait 4
open_requester 223 64 0
SCREEN_FIRST=$(info 223 64 0)
if [ "$OPENED" = container ]; then
    echo "  the screen opened: $(ctl screen | jq -r '.screen'), slots $(ctl screen | jq -r '(.slots | length)')"
    # Both panels are centred in the same GUI, so their tops differ by half their heights:
    # (238 - 208) / 2 = 15. That is the requester's declared height against the machine's 208.
    [ "$(( MACHINE_OY - OY ))" = 15 ] || fail "the requester's panel top is $OY against the machine's $MACHINE_OY: not the 238-high panel (15 above a 208-high one)"
    # A cell's entry is made from the carried item, as upstream's ghost filter slot is: pick the
    # torch up onto the cursor, click the first cell, put it back.
    TORCH_SLOT=$(ctl screen | jq -r '[.slots[] | select(.item == "minecraft:torch")][0].slot // ""')
    [ -n "$TORCH_SLOT" ] || fail "the torch is not in the open menu's inventory: $(ctl screen | jq -c '.slots')"
    ctl click-slot "$TORCH_SLOT" >/dev/null
    ctl wait 3
    press_cell 0
    ctl click-slot "$TORCH_SLOT" >/dev/null
    ctl wait 3
    ctl close >/dev/null
    ctl wait 6
    SCREEN_FIRST=$(info 223 64 0)
fi
echo "  the screen: $OPENED; after the first cell's click: $SCREEN_FIRST"

if [ "$SELFTEST" = screen ]; then
    # Today's behaviour, deliberately asserted: a right click on the requester opens nothing
    # (`screen: playing`) and no screen can make a filter entry (`filters=0`). The package opens its
    # menu and the click makes the entry, so this inverted run has to end here, non-zero.
    case "$OPENED" in
        playing) ;;
        *) fail "SELFTEST=screen: the requester opens its filter screen ($OPENED) where today's defect opens none (playing), so the run must end non-zero" ;;
    esac
    case "$SCREEN_FIRST" in
        *"filters=0"*) ;;
        *) fail "SELFTEST=screen: a screen press made a filter entry (filters=1) where today's defect makes none, so the run must end non-zero: $SCREEN_FIRST" ;;
    esac
    fail "SELFTEST=screen: today's defect is not on this package"
fi
[ "$OPENED" = container ] || fail "the requester's filter screen did not open: $OPENED"
case "$SCREEN_FIRST" in
    *"filters=1 item=minecraft:torch min=0 max=0 batch=0"*) ;;
    *) fail "the screen's first cell did not take the torch: $SCREEN_FIRST" ;;
esac

# The value buttons, at the pixels the screen draws them: the band's own guard first (`min` cannot
# pass `max`, and `max` comes back down), then `max` to 8, `min` to 4 and `batch` to 4 — the brief's
# entry, made entirely through the menu.
open_requester 223 64 0
press_min_up
press_max_up
press_max_down
ctl wait 4
GUARD=$(info 223 64 0)
echo "  the band guard (min + at 0..0, then max + and -): $GUARD"
case "$GUARD" in
    *"min=0 max=0"*) ;;
    *) fail "a step of the screen's buttons left an invalid band: $GUARD" ;;
esac
for _ in 1 2 3 4 5 6 7 8; do press_max_up; done
for _ in 1 2 3 4; do press_min_up; done
for _ in 1 2 3 4; do press_batch_up; done
ctl screenshot "$PORT/../_shared/evidence/stepcrafter-e2e/screen-requester.png" >/dev/null 2>&1 || true
ctl close >/dev/null
ctl wait 6
SCREEN_SET=$(info 223 64 0)
echo "  the entry the screen made: $SCREEN_SET"
case "$SCREEN_SET" in
    *"filters=1 item=minecraft:torch min=4 max=8 batch=4"*) ;;
    *) fail "the screen's buttons did not set the entry the brief names (torch 4 8 4): $SCREEN_SET" ;;
esac

# A carried resource repoints a cell: the band and batch the screen just set stay with the slot, and
# only the resource changes (upstream's slot takes the resource clicked onto it).
ctl give minecraft:stick 1 >/dev/null
ctl wait 4
open_requester 223 64 0
STICK_SLOT=$(ctl screen | jq -r '[.slots[] | select(.item == "minecraft:stick")][0].slot // ""')
[ -n "$STICK_SLOT" ] || fail "the stick is not in the open menu's inventory"
ctl click-slot "$STICK_SLOT" >/dev/null
ctl wait 3
press_cell 0
ctl click-slot "$STICK_SLOT" >/dev/null
ctl wait 3
ctl close >/dev/null
ctl wait 6
REPOINTED=$(info 223 64 0)
echo "  after a stick onto the torch's cell: $REPOINTED"
case "$REPOINTED" in
    *"filters=1 item=minecraft:stick min=4 max=8 batch=4"*) ;;
    *) fail "the cell did not take the carried resource while keeping its band: $REPOINTED" ;;
esac

# The command and the screen are one saved container: the command adds a second cell beside the
# screen's, and the screen's remove button drops it again.
say "stepcrafter requester add 223 64 0 minecraft:torch 4 8 4"
CMD_CELL=$(reply '^stepcrafter requester 223 64 0 ')
echo "  the command's entry beside the screen's: $CMD_CELL"
case "$CMD_CELL" in
    *"filters=2"*) ;;
    *) fail "the command's entry did not join the screen's container: $CMD_CELL" ;;
esac
open_requester 223 64 0
press_cell 1
press_remove
ctl close >/dev/null
ctl wait 6
AFTER_REMOVE=$(info 223 64 0)
echo "  after the screen's remove button: $AFTER_REMOVE"
case "$AFTER_REMOVE" in
    *"filters=1 item=minecraft:stick"*) ;;
    *) fail "the screen's remove button did not drop the command's entry: $AFTER_REMOVE" ;;
esac

# The differential: the same requester one row over, never opened, has no entry — the screen's
# press is the only difference between them.
SCREEN_OTHER=$(info 223 64 4)
echo "  the unopened requester beside it: $SCREEN_OTHER"
case "$SCREEN_OTHER" in
    *"filters=0 item=none"*) ;;
    *) fail "a requester nobody opened carries an entry: $SCREEN_OTHER" ;;
esac

echo "== the Step Crafter's own panel: the target cell and the band's buttons =="
# 0.2.7's gap (notes.md §1/§6): the machine was configured only by command while the requester had
# its own panel. The machine now opens the package's own menu type: the 27 stock slots are still
# real menu slots, and the target cell and the band's buttons are drawn over them (a press is a menu
# button id: the client hit-tests it, the server applies it to the same compound the command writes).
for z in 0 2 4 6; do
    ctl command "execute in minecraft:the_end run setblock 225 64 $z minecraft:air" >/dev/null
done
ctl wait 6
ctl command "execute in minecraft:the_end run setblock 225 64 0 $BLOCK" >/dev/null
ctl command "execute in minecraft:the_end run setblock 225 64 4 $BLOCK" >/dev/null
ctl wait 8

FRESH_MACHINE=$(info 225 64 0)
echo "  the fresh machine: $FRESH_MACHINE"
case "$FRESH_MACHINE" in
    *"target=none min=- max=-"*) ;;
    *) fail "a fresh machine is not unconfigured: $FRESH_MACHINE" ;;
esac

open_machine() { # X Y Z -> sets OPENED and, when it opened, OX/OY
    ctl close >/dev/null 2>&1 || true
    ctl tp "$1.5" "$2" "$(( $3 + 2 )).5" >/dev/null
    ctl wait 6
    ctl look 180 0 >/dev/null
    ctl wait 2
    ctl look-at block "$1" "$2" "$3" south >/dev/null 2>&1 || true
    ctl interact "$1" "$2" "$3" south >/dev/null || fail "the machine refused the right click"
    ctl wait 6
    OPENED=$(ctl screen | jq -r '.screen')
    if [ "$OPENED" = container ]; then PANEL_H=208; panel_origin 126; fi
}

# The cell and the four buttons, at the pixels `src/screen.rs` lays them out at.
press_target_cell() { press 17 85; }
press_min_down() { press 36 104; }
press_min_up() { press 72 104; }
press_max_down() { press 116 104; }
press_max_up() { press 152 104; }

ctl command "clear" >/dev/null
ctl wait 4
ctl give minecraft:torch 1 >/dev/null
ctl wait 4
open_machine 225 64 0
MACHINE_PANEL=$(ctl screen)
MACHINE_PANEL_SLOTS=$(jq '[.slots[] | select(.slot >= 300)] | length' <<<"$MACHINE_PANEL")
MACHINE_PANEL_SCREEN=$OPENED
MACHINE_PANEL_OY=$OY
echo "  the machine's panel: $MACHINE_PANEL_SCREEN, $MACHINE_PANEL_SLOTS stock slots, top $MACHINE_PANEL_OY"
if [ "$OPENED" = container ]; then
    [ "$MACHINE_PANEL_SLOTS" = 27 ] \
        || fail "the machine's panel shows $MACHINE_PANEL_SLOTS stock slots, not its 27"
    # The target cell takes the carried item, as upstream's pattern slot takes the pattern: pick the
    # torch up onto the cursor, click the cell, put it back.
    TORCH_SLOT=$(ctl screen | jq -r '[.slots[] | select(.item == "minecraft:torch")][0].slot // ""')
    [ -n "$TORCH_SLOT" ] || fail "the torch is not in the open panel's inventory: $(ctl screen | jq -c '.slots')"
    ctl click-slot "$TORCH_SLOT" >/dev/null
    ctl wait 3
    press_target_cell
    ctl click-slot "$TORCH_SLOT" >/dev/null
    ctl wait 3
    ctl close >/dev/null
    ctl wait 6
fi
SCREEN_TARGET=$(info 225 64 0)
echo "  after the torch onto the target cell: $SCREEN_TARGET"

if [ "$SELFTEST" = crafterscreen ]; then
    # Today's behaviour, deliberately asserted: the panel has no target cell, so the carried torch
    # clicked where the cell now is changes nothing and the machine still has no target. This
    # package's cell takes it, so this inverted run has to end here, non-zero.
    case "$MACHINE_PANEL_SCREEN" in
        container) ;;
        *) fail "SELFTEST=crafterscreen: the machine's panel did not open ($MACHINE_PANEL_SCREEN), so today's defect cannot be asserted" ;;
    esac
    case "$SCREEN_TARGET" in
        *"target=none"*) ;;
        *) fail "SELFTEST=crafterscreen: the target cell took the carried torch where today's panel has no cell and sets no target, so the run must end non-zero: $SCREEN_TARGET" ;;
    esac
    fail "SELFTEST=crafterscreen: today's defect is not on this package"
fi
[ "$MACHINE_PANEL_SCREEN" = container ] || fail "the machine's own panel did not open: $MACHINE_PANEL_SCREEN"
case "$SCREEN_TARGET" in
    *"target=minecraft:torch"*) ;;
    *) fail "the target cell did not take the torch: $SCREEN_TARGET" ;;
esac
case "$SCREEN_TARGET" in
    *"cells=2 result=minecraft:torch:4"*) ;;
    *) fail "the target cell's item did not resolve into its step: $SCREEN_TARGET" ;;
esac

# The band's buttons, at the pixels the panel draws them: `max` up 16 times and `min` up 8 read
# 8..16, the brief's band, made entirely through the panel.
open_machine 225 64 0
for _ in $(seq 1 16); do press_max_up; done
for _ in $(seq 1 8); do press_min_up; done
ctl close >/dev/null
ctl wait 6
SCREEN_BAND=$(info 225 64 0)
echo "  the band the panel's buttons set: $SCREEN_BAND"
case "$SCREEN_BAND" in
    *"min=8 max=16"*) ;;
    *) fail "the panel's buttons did not set the band the brief names (8..16): $SCREEN_BAND" ;;
esac

# Down to 8..8 and a `min +` that would invert the band is refused: the number stays 8.
open_machine 225 64 0
for _ in $(seq 1 8); do press_max_down; done
press_min_up
ctl close >/dev/null
ctl wait 6
SCREEN_GUARD=$(info 225 64 0)
echo "  the band's guard (max - to 8..8, then min +): $SCREEN_GUARD"
case "$SCREEN_GUARD" in
    *"min=8 max=8"*) ;;
    *) fail "a step of the panel's buttons left an invalid band: $SCREEN_GUARD" ;;
esac

open_machine 225 64 0
ctl screenshot "$PORT/../_shared/evidence/stepcrafter-e2e/screen-crafter.png" >/dev/null 2>&1 || true
ctl close >/dev/null
ctl wait 4

# The differential: the same machine one row over, never opened, is still unconfigured — the panel's
# presses are the only difference between the two.
SCREEN_OTHER_MACHINE=$(info 225 64 4)
echo "  the machine nobody opened: $SCREEN_OTHER_MACHINE"
case "$SCREEN_OTHER_MACHINE" in
    *"target=none min=- max=-"*) ;;
    *) fail "a machine nobody opened is configured: $SCREEN_OTHER_MACHINE" ;;
esac

echo "== the machine's several patterns: two targets, each with its own band =="
# 0.2.9: the machine carries a saved list of pattern slots (upstream's 45-slot container), each with
# its own target, band, status and count. Two cells pressed on its panel make two entries; the tick
# walks them in slot order and each fills its own band out of the shared inventory.
press_target_cell_1() { press 35 85; } # the second cell's centre: x = 8 + 18 + 9, y = 76 + 9
# Clear every crafter and requester in reach: a requester left by an earlier phase would otherwise
# link this fresh machine and ask it for sticks, skewing the per-slot counts this leg asserts.
for x in 220 221 222 223 224 225 226; do
    for z in 0 1 2 3 4; do
        ctl command "execute in minecraft:the_end run setblock $x 64 $z minecraft:air" >/dev/null
    done
done
ctl wait 6
ctl command "execute in minecraft:the_end run kill @e[type=item,x=224,y=63,z=-1,dx=4,dy=4,dz=7]" >/dev/null 2>&1 || true
ctl wait 4
ctl command "execute in minecraft:the_end run setblock 225 64 0 $BLOCK" >/dev/null
ctl command "execute in minecraft:the_end run setblock 225 64 4 $BLOCK" >/dev/null
ctl wait 8

# The first cell takes a carried torch; the second takes a carried stick. Both are drawn cells of
# the same panel, so the row is the only way the second target exists.
ctl give minecraft:torch 1 >/dev/null
ctl wait 4
open_machine 225 64 0
TORCH_SLOT=$(ctl screen | jq -r '[.slots[] | select(.item == "minecraft:torch")][0].slot // ""')
[ -n "$TORCH_SLOT" ] || fail "the torch is not in the open panel: $(ctl screen | jq -c '.slots')"
ctl click-slot "$TORCH_SLOT" >/dev/null
ctl wait 3
press_target_cell
ctl click-slot "$TORCH_SLOT" >/dev/null
ctl wait 3
ctl give minecraft:stick 1 >/dev/null
ctl wait 4
STICK_SLOT=$(ctl screen | jq -r '[.slots[] | select(.item == "minecraft:stick")][0].slot // ""')
[ -n "$STICK_SLOT" ] || fail "the stick is not in the open panel: $(ctl screen | jq -c '.slots')"
ctl click-slot "$STICK_SLOT" >/dev/null
ctl wait 3
press_target_cell_1
ctl click-slot "$STICK_SLOT" >/dev/null
ctl wait 3
ctl close >/dev/null
ctl wait 6
TWO_TARGETS=$(info 225 64 0)
echo "  two cells pressed: $TWO_TARGETS"
case "$TWO_TARGETS" in
    *"targets=2"*) ;;
    *) fail "the two cells did not make two target slots: $TWO_TARGETS" ;;
esac
case "$TWO_TARGETS" in
    *"entries=minecraft:torch|"*"minecraft:stick|"*) ;;
    *) fail "the second cell did not add a stick entry: $TWO_TARGETS" ;;
esac

if [ "$SELFTEST" = patterns ]; then
    # Today's defect, deliberately asserted: one target for the whole machine. This package keeps
    # two slots, so the inverted run has to end here, non-zero.
    case "$TWO_TARGETS" in
        *"targets=1"*) ;;
        *) fail "SELFTEST=patterns: the machine kept two target slots (targets=2) where today's defect keeps one (targets=1), so the run must end non-zero: $TWO_TARGETS" ;;
    esac
    fail "SELFTEST=patterns: today's defect is not on this package"
fi

# Each slot's band is retuned by item; the other slot's band and pattern stay as they were.
say "stepcrafter configure-item 225 64 0 minecraft:torch 8 16"
reply '^stepcrafter configure-item 225 64 0 ' >/dev/null
say "stepcrafter configure-item 225 64 0 minecraft:stick 16 32"
reply '^stepcrafter configure-item 225 64 0 ' >/dev/null
load 225 64 0 minecraft:coal 8
load 225 64 0 minecraft:stick 8
load 225 64 0 minecraft:oak_planks 16
ctl wait 80
MULTI=$(info 225 64 0)
echo "  after 80 ticks, the two slots: $MULTI"
case "$MULTI" in
    *"entries=minecraft:torch|8|16|idle|16|16;minecraft:stick|16|32|idle|28|32"*) ;;
    *) fail "the two slots did not each fill their own band: $MULTI" ;;
esac
case "$MULTI" in
    *"minecraft:coal:4"*"minecraft:torch:16"*) ;;
    *) fail "the shared stock was not consumed per entry (4 coal for the torches, the planks for the sticks): $MULTI" ;;
esac

# The differential: the same spot with only the first cell pressed has one entry, and nothing ever
# maintains sticks — the second cell is the only difference between the two machines.
ctl give minecraft:torch 1 >/dev/null
ctl wait 4
open_machine 225 64 4
TORCH_SLOT=$(ctl screen | jq -r '[.slots[] | select(.item == "minecraft:torch")][0].slot // ""')
[ -n "$TORCH_SLOT" ] || fail "the torch is not in the differential panel: $(ctl screen | jq -c '.slots')"
ctl click-slot "$TORCH_SLOT" >/dev/null
ctl wait 3
press_target_cell
ctl click-slot "$TORCH_SLOT" >/dev/null
ctl wait 3
ctl close >/dev/null
ctl wait 6
say "stepcrafter configure 225 64 4 8 16"
reply '^stepcrafter configure 225 64 4 ' >/dev/null
load 225 64 4 minecraft:coal 8
load 225 64 4 minecraft:stick 8
ctl wait 80
ONE_TARGET=$(info 225 64 4)
echo "  the machine with only the first cell pressed: $ONE_TARGET"
case "$ONE_TARGET" in
    *"targets=1"*"entries=minecraft:torch|8|16|idle|16|16"*) ;;
    *) fail "the differential machine did not keep exactly one torch slot: $ONE_TARGET" ;;
esac
ONE_ENTRIES=$(sed -n 's/.* entries=\([^ ]*\).*/\1/p' <<<"$ONE_TARGET")
[ -z "${ONE_ENTRIES//[^;]/}" ] || fail "the differential machine has more than one entry: $ONE_ENTRIES"

echo "== queued asks: one crafter, two requesters, each served its own share =="
# The 0.2.6 gap (notes.md §6): the ask was one `Int` in the crafter's compound, so a second
# requester asking the same crafter was never queued - its ask was lost while the first's was
# outstanding - and both read the same single inventory as their own. The ask is now a list of
# (requester, item, amount) that the crafter serves in a fixed order and credits to the requester
# whose step it was.
for pos in 221 222 223; do
    for z in 0 2 4; do
        ctl command "execute in minecraft:the_end run setblock $pos 64 $z minecraft:air" >/dev/null
    done
done
ctl wait 6
ctl command "execute in minecraft:the_end run kill @e[type=item,x=219,y=63,z=-1,dx=6,dy=4,dz=7]" >/dev/null 2>&1 || true
ctl wait 4
ctl command "execute in minecraft:the_end run setblock 221 64 0 $BLOCK"
ctl wait 6
say "stepcrafter target 221 64 0 minecraft:torch"
reply '^stepcrafter target 221 64 0 ' >/dev/null
say "stepcrafter configure 221 64 0 0 0"
reply '^stepcrafter configure 221 64 0 ' >/dev/null
# Exactly one step's ingredients: 1 coal + 1 stick make 4 torches, and nothing more.
load 221 64 0 minecraft:coal 1
load 221 64 0 minecraft:stick 1
QUEUE_NEW=$(info 221 64 0)
echo "  one step's ingredients: $QUEUE_NEW"
case "$QUEUE_NEW" in
    *"stored=0"*"crafted=0"*"minecraft:coal:1"*"minecraft:stick:1"*) ;;
    *) fail "the queue's crafter was not stocked with exactly one step: $QUEUE_NEW" ;;
esac
ctl command "execute in minecraft:the_end run setblock 222 64 0 $REQUESTER"
ctl command "execute in minecraft:the_end run setblock 222 64 2 $REQUESTER"
ctl wait 8
say "stepcrafter requester 222 64 0 minecraft:torch 8 8 4"
reply '^stepcrafter requester 222 64 0 ' >/dev/null
say "stepcrafter requester 222 64 2 minecraft:torch 8 8 4"
reply '^stepcrafter requester 222 64 2 ' >/dev/null
ctl wait 40
QUEUE_FIRST=$(info 222 64 0)
QUEUE_SECOND=$(info 222 64 2)
QUEUE_CRAFT=$(info 221 64 0)
echo "  after 40 ticks, the first:  $QUEUE_FIRST"
echo "  after 40 ticks, the second: $QUEUE_SECOND"
echo "  after 40 ticks, the crafter: $QUEUE_CRAFT"
# The first requester's four torches are its own; the second is queued behind it, still running,
# and its own share is zero - not the first's torches under another name.
case "$QUEUE_SECOND" in
    *"stored=0 served=0 queued=1 phase=running"*) ;;
    *) fail "the second requester is not queued behind the first with its own share of zero: $QUEUE_SECOND" ;;
esac
case "$QUEUE_FIRST" in
    *"stored=4 served=4"*) ;;
    *) fail "the first requester was not served its four torches: $QUEUE_FIRST" ;;
esac
case "$QUEUE_FIRST" in
    *"crafter=221,64,0"*) ;;
    *) fail "the first requester did not link the shared crafter: $QUEUE_FIRST" ;;
esac
case "$QUEUE_CRAFT" in
    *"stored=4"*"crafted=4"*"queued=2"*) ;;
    *) fail "the crafter did not hold both asks after serving the first: $QUEUE_CRAFT" ;;
esac
case "$QUEUE_CRAFT" in
    *"asks=222,64,0|minecraft:torch|4;222,64,2|minecraft:torch|4"*) ;;
    *) fail "the crafter's queue is not the two asks in their served order: $QUEUE_CRAFT" ;;
esac
case "$QUEUE_CRAFT" in
    *"served=222,64,0|minecraft:torch|4"*) ;;
    *) fail "the crafter did not credit the first step to the first requester: $QUEUE_CRAFT" ;;
esac
# A restock of two steps: the first's kept ask is served (it reaches its maximum of 8), then the
# second's - whose own share of the stock becomes four, the number the ask named.
load 221 64 0 minecraft:coal 2
load 221 64 0 minecraft:stick 2
ctl wait 40
QUEUE_FIRST_SETTLED=$(info 222 64 0)
QUEUE_SECOND_SETTLED=$(info 222 64 2)
QUEUE_CRAFT_SETTLED=$(info 221 64 0)
echo "  after the restock, the first:  $QUEUE_FIRST_SETTLED"
echo "  after the restock, the second: $QUEUE_SECOND_SETTLED"
echo "  after the restock, the crafter: $QUEUE_CRAFT_SETTLED"
case "$QUEUE_FIRST_SETTLED" in
    *"stored=8 served=8"*"phase=idle"*) ;;
    *) fail "the first requester did not reach its maximum after the restock: $QUEUE_FIRST_SETTLED" ;;
esac
case "$QUEUE_SECOND_SETTLED" in
    *"stored=4 served=4"*) ;;
    *) fail "the second requester was not served its four torches after the restock: $QUEUE_SECOND_SETTLED" ;;
esac
case "$QUEUE_CRAFT_SETTLED" in
    *"stored=12"*"crafted=12"*) ;;
    *) fail "the crafter did not make three steps for the two askers: $QUEUE_CRAFT_SETTLED" ;;
esac

if [ "$SELFTEST" = queue ]; then
    # Today's behaviour, deliberately asserted: with one `Int` for the ask, the second requester is
    # never queued - it reads the first's torches as its own stock and its ask is lost. This package
    # queues it and serves it its own four, so this inverted run has to end here, non-zero.
    case "$QUEUE_SECOND_SETTLED" in
        *"served=4"*) fail "SELFTEST=queue: the second requester was served its own four torches where today's single ask loses it, so the run must end non-zero: $QUEUE_SECOND_SETTLED" ;;
        *) ;;
    esac
    fail "SELFTEST=queue: today's defect is not on this package"
fi

echo "== /stepcrafter needs an operator =="
PLAYER=$(ctl state | jq -r '.player.name')
[ -n "$PLAYER" ] || fail "cannot name the local player"
ctl command "deop $PLAYER" >/dev/null
ctl wait 10 >/dev/null
say "stepcrafter info 202 64 0"
ctl wait 10 >/dev/null
REFUSAL=$(ctl messages 16 | jq -r '.chat[]' | grep -m1 -E 'operator|Unknown or incomplete command|permission' || true)
[ -n "$REFUSAL" ] || fail "a non-operator read a machine's state: $(ctl messages 16 | jq -r '.chat[]' | tail -3)"
ctl command "op $PLAYER" >/dev/null
ctl wait 6 >/dev/null
expect 202 64 0 "stepcrafter 202 64 0"
echo "  as a non-operator: $REFUSAL"

echo "PASS: the block and item registered and an unported id was refused, both carried recipes craft and consume their five ingredients, a fresh machine reads unconfigured, target/configure resolve the torch's own recipe and refuse a bedrock target, an unknown id and an inverted band, the container stocked 8 coal and 8 sticks and the tick filled the target 0->16 in steps of 4 consuming 4 and 4, it stopped at the maximum, take moved the target to the inventory, a stick-less step reported not-enough-ingredients and the restock resumed to 32, a manual step crafted and then refused with no ingredients, the 64..128 band filled past the minimum to the maximum with active true then false, a requester beside a quiet machine asked it to 8 torches in batches of 4 (stored=8 crafted=8) while the same world without the requester left a second machine at 0, the refusal reported not-enough-ingredients and the restock resumed it, the requester's active state followed its run, a crafter whose slots left no room for the result (26x64 coal + 60 sticks) refused every ask as network-full with no ingredient consumed and nothing crafted while the requester read stored=0, a named stack of the target was no room for the step's plain result (network-full, crafted=0, stock unchanged) while the same step beside a plain stack merged its four torches into it, a requester with two filter entries read filters=2 and asked its torch crafter to 8 and its stick crafter to 16 while the same world with only one entry left the stick crafter at 0, take handed over the stored stacks with their custom names and damage intact (Alpha, Beta, damage 7), a cake step made the cake and left the three buckets its milk leaves, two requesters shared one crafter with the second queued behind the first and each served its own four after a restock, the requester's screen opened on a right click, its first cell took a carried torch, its drawn buttons set that entry to torch 4 8 4 (the band guard refused min + at 0..0), a carried stick repointed the cell keeping its band, a command-added entry appeared as a cell the remove button dropped, and a requester nobody opened carried no entry, the machine opened its own 176x208 panel on a right click with its 27 stock slots still there, a carried torch on its target cell read target=minecraft:torch cells=2 result=minecraft:torch:4, its drawn max + x16 and min + x8 read min=8 max=16 while a min + at 8..8 was refused, and a machine nobody opened read target=none min=- max=-, its refusals and its own loot held, a broken machine dropped its own item and spilled its stock (and a moved machine is re-targeted, as recorded), and the machine's two target cells made two saved slots that each filled their own band out of the shared stock (torch 8..16, stick 16..32, four coal and sixteen planks consumed) with per-entry targets=2 entries=..., while the same spot with only the first cell pressed kept one slot, and the command needs an operator"
