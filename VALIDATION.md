# Step Crafter — validation (0.2.5)

Everything here is a command that was run on 2026-11-23, with its artefact under
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice is **the Step Requester's several filter entries** (`notes.md` §2f): upstream's `rf`
container is 45 entries, each a resource with its own band, batch, status and failed-task timeout;
0.2.4 kept one. The package is **62,413 B** sha256
`70e238c3a262ee33e31db0ab10afc44dc14e7655189a258fc56b411413b34525`, version `0.2.5`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red (unit) | `cargo test --offline` with `Filters::set` keeping only the newest entry | `FAILED. 40 passed; 5 failed` (exit 101): `left: Ok(0) / right: Ok(1)`, `left: 1 / right: 45`, `left: 0 / right: 1`, `left: 1 / right: 2` (the container's saved form), and the malformed-link guard | `stepcrafter/red-unit-filters.log` |
| red (game) | `SELFTEST=filters bash tools/e2e.sh` on the fixed package | `FAIL: SELFTEST=filters: the requester keeps both filter entries (filters=2) where today's defect keeps one (filters=1) …` | `stepcrafter-e2e/e2e-selftest-filters.log` |
| unit tests | `cargo test --offline` | **46 passed, 0 failed** | `stepcrafter/sdk-test.log` |
| format | `cargo fmt --check` on the port's own sources (not `--all`: it would reach into `.horizon/sdk`) | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, copying `target/horizon-mod/stepcrafter.hmod` | `cmp` identical, sha256 `70e238c3…` twice, **0** `Cargo.lock` entries, 37 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.5 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `62413 bytes, SHA-256 70e238c3…`, `EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** (also `package matches the tree`) | `stepcrafter/verify-sdk-package.log` |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=9 ok=9 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`70e238c3…`).

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/build-slot.sh run --job W1-stepcrafter --port ports/stepcrafter --step build --
ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-8fd30e1c035b built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The version bump made
it recompile, so the exported binary is the delivered sources, not a reused manifest.

Each run is one command, through the slot registry, with the harness's own scratch root
(`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`):

| run | command | verdict | log |
|---|---|---|---|
| main | `launcher-game.sh run stepcrafter` (`GAME` = the built game, in-run `CONTROL_GAME` leg) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control | `GAME=$CONTROL_GAME bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`expected stepcrafter:step_crafter at 200 64 0, got minecraft:air`) | `stepcrafter-e2e/e2e-control.log` |
| selftest (requester) | `SELFTEST=1 bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`the requester reached stored=8 …`) | `stepcrafter-e2e/e2e-selftest.log` |
| selftest (no room) | `SELFTEST=capacity …` | `FAIL` + `EXIT:1` (`the step fit no slot and served nothing …`) | `stepcrafter-e2e/e2e-selftest-capacity.log` |
| selftest (components) | `SELFTEST=components-room …` | `FAIL` + `EXIT:1` (`the named stack counted no room and the step was refused …`) | `stepcrafter-e2e/e2e-selftest-components-room.log` |
| selftest (take) | `SELFTEST=take …` | `FAIL` + `EXIT:1` (`the taken stacks kept their components …`) | `stepcrafter-e2e/e2e-selftest-take.log` |
| selftest (remainders) | `SELFTEST=remainder …` | `FAIL` + `EXIT:1` (`the step left the buckets its milk leaves …`) | `stepcrafter-e2e/e2e-selftest-remainder.log` |
| **selftest (filters)** | `SELFTEST=filters …` | `FAIL` + `EXIT:1` (`the requester keeps both filter entries (filters=2) where today's defect keeps one (filters=1) …`) | `stepcrafter-e2e/e2e-selftest-filters.log` |
| red (old package) | the 0.2.3 export run with the 0.2.4 leg, no rebuild | `FAIL` + `EXIT:1` (`result=crafted` beside the unchanged named stack) | `stepcrafter-e2e/e2e-red-components-0.2.3.log` |

```
$ python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict
ok       stepcrafter-e2e        e2e-control.log            FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-launcher.log           PASS + EXIT:0
ok       stepcrafter-e2e        e2e-red-components-0.2.3.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-capacity.log  FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-components-room.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-filters.log   FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-remainder.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-take.log      FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest.log           FAIL + EXIT:1
logs=9  ok=9  missing-exit=0  bad-exit=0
```

## The slice: several filter entries, in game

One requester at `222 64 0` carries two entries; the torch crafter is at `221 64 0` (4 coal + 4
sticks), the stick crafter at `223 64 0` (8 `minecraft:oak_planks`):

```
  stepcrafter requester add 222 64 0 minecraft:torch 4 8 4   -> … filters=1
  stepcrafter requester add 222 64 0 minecraft:stick 8 16 8  -> … filters=2
  after 80 ticks, the requester:
    filters=2 item=minecraft:torch … stored=8 … requests=4
    entries=minecraft:torch|4|8|4|8|idle|221,64,0|0;minecraft:stick|8|16|8|16|idle|223,64,0|0
  the torch crafter: … stored=8 crafted=8 request=0 stock=minecraft:coal:2,minecraft:stick:2,minecraft:torch:8
  the stick crafter: … stored=16 crafted=16 request=0 stock=minecraft:stick:16
```

The **differential** is a second requester at `222 64 4` with only the torch entry (the 0.2.1–0.2.4
spelling), beside a fresh torch crafter `221 64 4` and stick crafter `223 64 4` stocked identically:

```
  the one-entry requester: … filters=1 …
  the torch crafter: … crafted=8 …
  the stick crafter: … crafted=0 …
```

Only the removed entry differs, so the assertion above can fail. `SELFTEST=filters` asserts the old
one-entry reply and ends the run non-zero. The same step drives the requester's saved container, the
per-entry link, the per-entry backoff and the readout's `entries=` list.

**A scenario correction from the first run:** the brief's "8 planks" cannot be stocked as
`minecraft:planks` — that is a tag, not an item id, and `ctl container` silently stocks nothing for
it, so the stick crafter never stepped. The leg names `minecraft:oak_planks` and asserts
`stock=minecraft:oak_planks:8` before the filters, so the mistake fails at the point it is made.

## Coverage (family → evidence)

| family | evidence |
|---|---|
| the machine's band rule (start < min, fill to max, hold) | `runtime::tests::decide_*`; in game: `stored=16 crafted=16` then 60 idle ticks |
| one step = cells out, result in | `a_step_consumes_every_cell_and_makes_the_result`; in game: 4 coal + 4 sticks → 16 torches |
| what the ingredients leave | `a_step_returns_the_bucket_the_recipe_leaves`, `a_remainder_that_fits_no_slot_refuses_the_step`, `an_empty_slot_offers_each_inserted_item_its_own_room`; in game: cake step → `bucket:3` + `cake:1`; `SELFTEST=remainder` ends non-zero |
| `not-enough-ingredients` | `a_missing_ingredient_refuses_the_step_and_names_the_cell`; in game: `phase=not-enough-ingredients`, then resume to `crafted=32`; a manual refused step records the same phase |
| `network-full` (per-slot room, post-consumption) | `a_full_inventory_refuses_the_step`, `a_result_that_fits_no_slot_…`, `a_slot_holding_the_result_offers_only_its_own_free_room`, `consuming_the_last_item_of_a_slot_frees_that_slot_for_the_result`; in game: the 1,724-item machine refuses every ask, `crafted=0`, stock unchanged |
| the room for a plain insert beside a named stack | `a_named_stack_of_the_result_is_no_room_for_a_plain_step`, `a_plain_result_is_a_new_stack_beside_a_named_one`; in game: 221 refuses (`phase=network-full crafted=0`), 222 merges 4; `SELFTEST=components-room` ends non-zero |
| the manual step | `a_manual_step_*`; in game: `result=crafted` / `result=not-enough-ingredients` with the phase recorded |
| the `active` state | in game: `[active=true]` → `Test passed` while filling, `[active=false]` once idle |
| the container (stock, spill) | in game: `ctl container` loads 8+8; a survival break drops `minecraft:torch:8` + `stepcrafter:step_crafter:1` |
| the recipes (both) | `tools/craft_check.py`: one output each, five ingredients consumed each, wrong pattern crafts nothing |
| the blocks + items (both) | placed and given in game; `stepcrafter:step_crafter_manager` refused by both |
| the commands | every reply parsed: `info`/`target`/`configure`/`requester add|remove`/`step`/`take`, and the refusals |
| `take` (the stored stacks, components and damage) | in game: Alpha/Beta named stacks, a Blade sword with `damage 7` and `sharpness 3`; `SELFTEST=take` ends non-zero |
| the tag (`mineable/pickaxe`) | carried for both blocks; the survival break with a diamond pickaxe drops them (a bare hand does not) |
| the requester's rule | `a_requester_*`, `a_batch_caps_each_request`, `a_request_is_not_repeated_while_one_is_outstanding`, `a_failed_task_backs_off_before_asking_again`, `a_zero_batch_filter_is_disabled`, `a_crafter_that_refused_is_the_requesters_failed_task`; in game `requests=2 → stored=8` and `phase=running` at `stored=44` on the 32-step run |
| **several filter entries, each served by its own crafter** | `a_second_filter_entry_is_kept_beside_the_first`, `a_filter_entry_is_replaced_by_its_own_resource`, `a_requester_carries_at_most_the_containers_45_entries`, `a_removed_entry_leaves_the_others`, `a_filter_entry_round_trips_through_its_saved_form`, `the_filter_container_round_trips_through_its_saved_form`, `a_legacy_single_filter_compound_still_reads_as_one_entry`; in game: `filters=2` with both entries and their stored counts, `crafted=8` + `crafted=16`, while one entry in the same world leaves the stick crafter at `crafted=0`; `SELFTEST=filters` ends non-zero |
| the interop (requester → crafter) | in game: `stored=8 crafted=8`; the same world without the requester leaves a machine at `stored=0 crafted=0` |
| the cross-block refusal | in game: `phase=not-enough-ingredients backoff=13`, restock → `stored=8 crafted=8`; and the no-room refusal reads `phase=network-full` on both blocks |
| colouring recipes, tags, advancements, managers, monitor, slot upgrade, GUI | `not ported` / `blocked(no-parent-addon-surface)` — `notes.md` §1 |
