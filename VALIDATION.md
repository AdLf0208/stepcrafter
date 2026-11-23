# Step Crafter — validation (0.2.2)

Everything here is a command that was run on 2026-11-23, with its artefact under
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice is the **code review** (`levzzz5154`'s static review of `c8a6e83e`): the room check is now
per slot (post-consumption), `take` keeps item components, and the archive carries the upstream
notice and the tree's current README (`notes.md` §2c). The package is **52,361 B** sha256
`d7f5eb112ed6bd2e06d4ae83432a4744bc3a0f32cf849ed65a3a59f0d5d7bff5`, version `0.2.2`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red (room) | `cargo test --offline` with `runtime::craft` still making the review's aggregate check | `FAILED. 32 passed; 2 failed` (exit 101): `left: Craft` / `right: NetworkFull { …, room: 0, produced: 4 }` | `stepcrafter/red-unit-capacity.log` |
| unit tests | `cargo test --offline` | **34 passed, 0 failed** | `stepcrafter/sdk-test.log` |
| format | `cargo fmt --check` on the port's own sources | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, copying `target/horizon-mod/stepcrafter.hmod` | `cmp` identical, sha256 `d7f5eb11…` twice, **0** `Cargo.lock` entries, 37 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.2 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `52361 bytes, SHA-256 d7f5eb11…`, `EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** | `stepcrafter/verify-sdk-package.log` |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=5 ok=5 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`d7f5eb11…`).

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/build-slot.sh run --job W1-stepcrafter --port ports/stepcrafter --step build --
ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-812dda2169ac built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The version bump made
it recompile, so the exported binary is the delivered sources, not a reused manifest.

Each run is one command, through the slot registry, with the harness's own scratch root
(`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`):

| run | command | verdict | log |
|---|---|---|---|
| main | `build-slot.sh run --step e2e -- bash ports/stepcrafter/tools/e2e.sh` (`GAME` = the built game, `CONTROL_GAME` = the same Nightly without the package) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control | `GAME=$CONTROL_GAME bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`block(200,64,0): minecraft:air` — the id is unknown without the package) | `stepcrafter-e2e/e2e-control.log` |
| selftest (requester) | `SELFTEST=1 bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`SELFTEST=1: the requester reached stored=8 …`) | `stepcrafter-e2e/e2e-selftest.log` |
| selftest (room) | `SELFTEST=capacity bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`SELFTEST=capacity: the step fit no slot and served nothing …`) | `stepcrafter-e2e/e2e-selftest-capacity.log` |
| selftest (components) | `SELFTEST=take bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`SELFTEST=take: the taken stacks kept their components …`) | `stepcrafter-e2e/e2e-selftest-take.log` |

```
$ python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict
ok       stepcrafter-e2e        e2e-control.log            FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-launcher.log           PASS + EXIT:0
ok       stepcrafter-e2e        e2e-selftest-capacity.log  FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-take.log      FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest.log           FAIL + EXIT:1
logs=5  ok=5  missing-exit=0  bad-exit=0
```

## The review's three points, answered

1. **Room, not budget** (`runtime.rs`): `Slots`/`Slot` model the 27 slots with each stack's item,
   count and stack limit; `plan`/`craft` take the ingredients from a copy of them and commit only when
   the result's post-consumption room holds all of it. The red test is
   `a_result_that_fits_no_slot_is_refused_even_when_the_counts_add_up` (1,724 items in 27 full-ish
   slots); in game, the same machine refused every ask at `items=1724` with `crafted=0` and its stock
   unchanged, and the requester read `stored=0 phase=network-full`.
2. **Components survive `take`** (`commands.rs`): the stored stacks themselves are handed over, split
   to the count, remainder kept in its slot; `machine::put_into_slots` merges only component-matching
   stacks. In game, Alpha ×2 + Beta ×3 named torches and a Blade sword at `damage 7` with
   `sharpness 3` came out with all of it; `SELFTEST=take` asserts the defect and ends non-zero.
3. **Notice and README** (archive): `LICENSE` is upstream's MIT text verbatim; `LICENSE-NOTICE.md`
   names **Ultramega**, **Starforcraft** and `https://github.com/starforcraft/Step-Crafter` verbatim;
   both are packed `cmp`-identical to the tree's copies, and so is `README.md`. The registry entry is
   the orchestrator's and is not touched here.

## Coverage (family → evidence)

| family | evidence |
|---|---|
| the machine's band rule (start < min, fill to max, hold) | `runtime::tests::decide_*`; in game: `stored=16 crafted=16` then 60 idle ticks |
| one step = cells out, result in | `a_step_consumes_every_cell_and_makes_the_result`; in game: 4 coal + 4 sticks → 16 torches |
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
