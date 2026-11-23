# Step Crafter — validation (0.2.3)

Everything here is a command that was run on 2026-11-23, with its artefact under
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice is **crafting remainders** (`notes.md` §2d): a step now leaves what its ingredients leave,
so a cake step returns its three milk buckets instead of destroying them. The package is **55,800 B**
sha256 `ebbdd9cb13d44f8bd8d0319e86053dd24dd7f3aba832b1ca97f09eed39ef29c5`, version `0.2.3`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| symbol check | `grep` over `ports/_shared/sdk/0a875da` for a remainder accessor | `Host::item_crafting_remainder` (`host/block_states.rs:44`) and `RecipeMatch.remainders` (`item/crafting.rs:230`) both exist → dispatchable, no `blocked(...)` | `notes.md` §2d |
| red (remainders) | `cargo test --offline` with `craft` still returning no remainder | `FAILED. 35 passed; 2 failed` (exit 101): `left: Craft { …, remainders: [] }` / `right: … remainders: [("minecraft:bucket", 3)]` | `stepcrafter/red-unit-remainder.log` |
| unit tests | `cargo test --offline` | **37 passed, 0 failed** | `stepcrafter/sdk-test.log` |
| format | `cargo fmt --check` on the port's own sources (not `--all`: it would reach into `.horizon/sdk`) | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, copying `target/horizon-mod/stepcrafter.hmod` | `cmp` identical, sha256 `ebbdd9cb…` twice, **0** `Cargo.lock` entries, 37 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.3 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `55800 bytes, SHA-256 ebbdd9cb…`, `EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** | `stepcrafter/verify-sdk-package.log` |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=6 ok=6 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`ebbdd9cb…`).

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/build-slot.sh run --job W1-stepcrafter --port ports/stepcrafter --step build --
ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-c3bd9255049c built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The version bump made
it recompile, so the exported binary is the delivered sources, not a reused manifest.

Each run is one command, through the slot registry, with the harness's own scratch root
(`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`):

| run | command | verdict | log |
|---|---|---|---|
| main | `launcher-game.sh run stepcrafter` (`GAME` = the built game, `CONTROL_GAME` = the same Nightly without the package) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control | `GAME=$CONTROL_GAME bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`FAIL: expected stepcrafter:step_crafter at 200 64 0, got minecraft:air` — the id is unknown without the package) | `stepcrafter-e2e/e2e-control.log` |
| selftest (requester) | `SELFTEST=1 bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`SELFTEST=1: the requester reached stored=8 …`) | `stepcrafter-e2e/e2e-selftest.log` |
| selftest (room) | `SELFTEST=capacity bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`SELFTEST=capacity: the step fit no slot and served nothing …`) | `stepcrafter-e2e/e2e-selftest-capacity.log` |
| selftest (components) | `SELFTEST=take bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`SELFTEST=take: the taken stacks kept their components …`) | `stepcrafter-e2e/e2e-selftest-take.log` |
| selftest (remainders) | `SELFTEST=remainder bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`SELFTEST=remainder: the step left the buckets its milk leaves …`) | `stepcrafter-e2e/e2e-selftest-remainder.log` |

```
$ python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict
ok       stepcrafter-e2e        e2e-control.log            FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-launcher.log           PASS + EXIT:0
ok       stepcrafter-e2e        e2e-selftest-capacity.log  FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-remainder.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-take.log      FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest.log           FAIL + EXIT:1
logs=6  ok=6  missing-exit=0  bad-exit=0
```

## The slice: crafting remainders, in game

The leg (machine at 220, band `0..0` so nothing fires by itself):

```
  stepcrafter target 220 64 0 target=minecraft:cake cells=9 result=minecraft:cake:1 remainders=minecraft:milk_bucket=minecraft:bucket:1
  before the step: … stored=0 … items=9 stock=minecraft:egg:1,minecraft:milk_bucket:3,minecraft:sugar:2,minecraft:wheat:3
  stepcrafter step 220 64 0 result=crafted
  after the step:  … stored=1 phase=idle … result=minecraft:cake:1 crafted=1 items=4 stock=minecraft:bucket:3,minecraft:cake:1
```

The cake **and** the three buckets, with every ingredient gone — the assertion reads the whole
`stock=` string, so a leftover ingredient or a missing bucket fails it. `SELFTEST=remainder` asserts
the old behaviour (no bucket in the stock) and the run ends `EXIT:1`.

## Coverage (family → evidence)

| family | evidence |
|---|---|
| the machine's band rule (start < min, fill to max, hold) | `runtime::tests::decide_*`; in game: `stored=16 crafted=16` then 60 idle ticks |
| one step = cells out, result in | `a_step_consumes_every_cell_and_makes_the_result`; in game: 4 coal + 4 sticks → 16 torches |
| **what the ingredients leave** | `a_step_returns_the_bucket_the_recipe_leaves`, `a_remainder_that_fits_no_slot_refuses_the_step`, `an_empty_slot_offers_each_inserted_item_its_own_room`, the saved `Remainders` round trip; in game: cake step → `bucket:3` + `cake:1`, ingredients consumed; `SELFTEST=remainder` ends non-zero |
| `not-enough-ingredients` | `a_missing_ingredient_refuses_the_step_and_names_the_cell`; in game: `phase=not-enough-ingredients`, then resume to `crafted=32` |
| `network-full` (per-slot room, post-consumption) | `a_full_inventory_refuses_the_step`, `a_result_that_fits_no_slot_…`, `a_slot_holding_the_result_offers_only_its_own_free_room`, `consuming_the_last_item_of_a_slot_frees_that_slot_for_the_result`; in game: the 1,724-item machine refuses every ask, `crafted=0`, stock unchanged |
| the manual step | `a_manual_step_*`; in game: `result=crafted` / `result=not-enough-ingredients` |
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
