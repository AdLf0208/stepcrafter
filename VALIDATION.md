# Step Crafter — validation (0.2.4)

Everything here is a command that was run on 2026-11-23, with its artefact under
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice is **the component-aware room for a step** (`notes.md` §2e): a stack carrying a component
patch (a name, an enchantment, damage) is no room for the step's plain result, and the commit is a
dry run through the real merge rule. The package is **57,384 B** sha256
`675a229bb84827f55df67283c3b0080f1c489c0144a0b9e30966d8ad5a33e33e`, version `0.2.4`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red (unit) | `cargo test --offline` after the two new tests, with `Slot::patched`'s flag unused | `FAILED. 37 passed; 2 failed` (exit 101): `left: 4 / right: 0` (room) and `left: [(64, false)] / right: [(60, false), (4, true)]` | `stepcrafter/red-unit-components.log` (the two-line compile red that named the API first: `red-unit-components-compile.log`) |
| red (game) | the **0.2.3** export run with 0.2.4's leg (`bash tools/e2e.sh`, no rebuild) | `FAIL: a step whose result only fits a named stack was not refused: … result=crafted` — the named stack stayed `torch:60` | `stepcrafter-e2e/e2e-red-components-0.2.3.log` |
| unit tests | `cargo test --offline` | **39 passed, 0 failed** | `stepcrafter/sdk-test.log` |
| format | `cargo fmt --check` on the port's own sources (not `--all`: it would reach into `.horizon/sdk`) | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, copying `target/horizon-mod/stepcrafter.hmod` | `cmp` identical, sha256 `675a229b…` twice, **0** `Cargo.lock` entries, 37 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.4 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `57384 bytes, SHA-256 675a229b…`, `EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** (also `package matches the tree`) | `stepcrafter/verify-sdk-package.log` |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=8 ok=8 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`675a229b…`).

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/build-slot.sh run --job W1-stepcrafter --port ports/stepcrafter --step build --
ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-4e8bcf54f030 built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The version bump made
it recompile, so the exported binary is the delivered sources, not a reused manifest.

Each run is one command, through the slot registry, with the harness's own scratch root
(`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`):

| run | command | verdict | log |
|---|---|---|---|
| main | `launcher-game.sh run stepcrafter` (`GAME` = the built game, in-run `CONTROL_GAME` leg) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control | `GAME=$CONTROL_GAME bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`expected stepcrafter:step_crafter at 200 64 0, got minecraft:air`) | `stepcrafter-e2e/e2e-control.log` |
| red (old package) | the 0.2.3 export run with this leg, no rebuild | `FAIL` + `EXIT:1` (`result=crafted` beside the unchanged named stack) | `stepcrafter-e2e/e2e-red-components-0.2.3.log` |
| selftest (requester) | `SELFTEST=1 bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`the requester reached stored=8 …`) | `stepcrafter-e2e/e2e-selftest.log` |
| selftest (no room) | `SELFTEST=capacity …` | `FAIL` + `EXIT:1` (`the step fit no slot and served nothing …`) | `stepcrafter-e2e/e2e-selftest-capacity.log` |
| selftest (components) | `SELFTEST=take …` | `FAIL` + `EXIT:1` (`the taken stacks kept their components …`) | `stepcrafter-e2e/e2e-selftest-take.log` |
| selftest (remainders) | `SELFTEST=remainder …` | `FAIL` + `EXIT:1` (`the step left the buckets its milk leaves …`) | `stepcrafter-e2e/e2e-selftest-remainder.log` |
| selftest (component room) | `SELFTEST=components-room …` | `FAIL` + `EXIT:1` (`the named stack counted no room and the step was refused …`) | `stepcrafter-e2e/e2e-selftest-components-room.log` |

```
$ python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict
ok       stepcrafter-e2e        e2e-control.log            FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-launcher.log           PASS + EXIT:0
ok       stepcrafter-e2e        e2e-red-components-0.2.3.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-capacity.log  FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-components-room.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-remainder.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-take.log      FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest.log           FAIL + EXIT:1
logs=8  ok=8  missing-exit=0  bad-exit=0
```

## The slice: the component-aware room, in game

The leg plants 27 occupied slots with `/data merge block` (the same block-entity write path the
requester uses), so no slot is empty and the named stack is the only candidate for the result:

```
  machine 221: slot 0 = 60 named torches ("Kept"), 25 x 64 coal, 1 x 64 stick (items=1724)
  before the step: … stored=60 … crafted=0 items=1724 stock=minecraft:coal:1600,minecraft:stick:64,minecraft:torch:60
  stepcrafter step 221 64 0 result=network-full
  after the step:  … stored=60 phase=network-full … crafted=0 items=1724 stock=minecraft:coal:1600,minecraft:stick:64,minecraft:torch:60
  machine 222: the same stacks with a plain torch stack of 60 (the differential)
  stepcrafter step 222 64 0 result=crafted
  after the step:  … stored=64 phase=idle … crafted=4 items=1726 stock=minecraft:coal:1599,minecraft:stick:63,minecraft:torch:64
```

The named machine consumed **nothing** and recorded **nothing**, and the plain twin merged its four
torches: only the component patch differs, so the assertion can fail. (The brief's literal
`Items` list had 26 coal stacks and no stick; the torch recipe's second cell is a stick, so that
list would stop at `not-enough-ingredients` before the room check. The leg keeps the named stack and
the full 27-slot inventory and adds the one stick the step needs.)

`SELFTEST=components-room` asserts the fixed refusal and exits non-zero; the 0.2.3 package's
`result=crafted` with the stock unchanged is `e2e-red-components-0.2.3.log`.

## Coverage (family → evidence)

| family | evidence |
|---|---|
| the machine's band rule (start < min, fill to max, hold) | `runtime::tests::decide_*`; in game: `stored=16 crafted=16` then 60 idle ticks |
| one step = cells out, result in | `a_step_consumes_every_cell_and_makes_the_result`; in game: 4 coal + 4 sticks → 16 torches |
| what the ingredients leave | `a_step_returns_the_bucket_the_recipe_leaves`, `a_remainder_that_fits_no_slot_refuses_the_step`, `an_empty_slot_offers_each_inserted_item_its_own_room`, the saved `Remainders` round trip; in game: cake step → `bucket:3` + `cake:1`, ingredients consumed; `SELFTEST=remainder` ends non-zero |
| `not-enough-ingredients` | `a_missing_ingredient_refuses_the_step_and_names_the_cell`; in game: `phase=not-enough-ingredients`, then resume to `crafted=32`; a manual refused step records the same phase |
| `network-full` (per-slot room, post-consumption) | `a_full_inventory_refuses_the_step`, `a_result_that_fits_no_slot_…`, `a_slot_holding_the_result_offers_only_its_own_free_room`, `consuming_the_last_item_of_a_slot_frees_that_slot_for_the_result`; in game: the 1,724-item machine refuses every ask, `crafted=0`, stock unchanged |
| **the room for a plain insert beside a named stack** | `a_named_stack_of_the_result_is_no_room_for_a_plain_step`, `a_plain_result_is_a_new_stack_beside_a_named_one`; in game: 221 refuses (`phase=network-full crafted=0`, stock unchanged), 222 merges 4 (`torch:64 crafted=4`); `SELFTEST=components-room` ends non-zero; the old package's loss is the red log |
| the manual step | `a_manual_step_*`; in game: `result=crafted` / `result=not-enough-ingredients` with the phase recorded |
| the `active` state | in game: `[active=true]` → `Test passed` while filling, `[active=false]` once idle |
| the container (stock, spill) | in game: `ctl container` loads 8+8; a survival break drops `minecraft:torch:8` + `stepcrafter:step_crafter:1` |
| the recipes (both) | `tools/craft_check.py`: one output each, five ingredients consumed each, wrong pattern crafts nothing |
| the blocks + items (both) | placed and given in game; `stepcrafter:step_crafter_manager` refused by both |
| the commands | every reply parsed: `info`/`target`/`configure`/`requester`/`step`/`take`, and the refusals |
| `take` (the stored stacks, components and damage) | in game: Alpha/Beta named stacks, a Blade sword with `damage 7` and `sharpness 3`, read back from `ctl inventory`; `SELFTEST=take` ends non-zero |
| the tag (`mineable/pickaxe`) | carried for both blocks; the survival break with a diamond pickaxe drops them (a bare hand does not) |
| the requester's rule | `a_requester_*`, `a_batch_caps_each_request`, `a_request_is_not_repeated_while_one_is_outstanding`, `a_failed_task_backs_off_before_asking_again`, `a_zero_batch_filter_is_disabled`, `a_crafter_that_refused_is_the_requesters_failed_task`; in game `requests=2 → stored=8` and `phase=running` at `stored=44` on the 32-step run |
| the interop (requester → crafter) | in game: `stored=8 crafted=8`; the same world without the requester leaves a machine at `stored=0 crafted=0` |
| the cross-block refusal | in game: `phase=not-enough-ingredients backoff=13`, restock → `stored=8 crafted=8`; and the no-room refusal reads `phase=network-full` on both blocks |
| colouring recipes, tags, advancements, managers, monitor, slot upgrade, GUI | `not ported` / `blocked(no-parent-addon-surface)` — `notes.md` §1 |
