# Step Crafter — validation (0.2.1)

Everything here is a command that was run on 2026-11-22, with its artefact under
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice adds the **Step Requester** (`notes.md` §2b); the package is
**49,241 B** sha256 `b40307c5ddf08c4ea9f7d6e358248dff1d27c5774b41e46eabcbcca6cdc6de63`, version
`0.2.1`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red (requester) | `cargo test --offline` with `runtime::ask` stubbed to `Hold` | `FAILED. 19 passed; 6 failed` (exit 101) | `stepcrafter/red-unit-requester.log` |
| unit tests | `cargo test --offline` | **30 passed, 0 failed** | `stepcrafter/sdk-test.log` |
| format | `cargo fmt --check` | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, copying `target/horizon-mod/stepcrafter.hmod` | `cmp` identical, sha256 `b40307c5…` twice, **0** `Cargo.lock` entries, 37 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.1 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `49241 bytes, SHA-256 b40307c5…`, `EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** | this file's §gate |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=3 ok=3 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`b40307c5…`). `fits` resolves its build list over
the unauthenticated GitHub API and answered `403` / `TLS reset` on the first two attempts; both runs
were repeated when the quota and the connection returned, and both are saved green.

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/build-slot.sh run --job W1-stepcrafter --port ports/stepcrafter --step build --
ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-dfe3f967f5d4 built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The version bump made
it recompile, so the exported binary is the delivered sources, not a reused manifest.

Each run is one command, through the slot registry, with the harness's own scratch root
(`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`):

| run | command | verdict | log |
|---|---|---|---|
| main | `build-slot.sh run --step e2e -- bash ports/stepcrafter/tools/e2e.sh` (`GAME` = the built game, `CONTROL_GAME` = the same Nightly without the package) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control | `GAME=$CONTROL_GAME bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`block(200,64,0): minecraft:air` — the id is unknown without the package) | `stepcrafter-e2e/e2e-control.log` |
| selftest | `SELFTEST=1 bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`SELFTEST=1: the requester reached stored=8 …`) | `stepcrafter-e2e/e2e-selftest.log` |

```
$ python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict
ok       stepcrafter-e2e        e2e-control.log            FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-launcher.log           PASS + EXIT:0
ok       stepcrafter-e2e        e2e-selftest.log           FAIL + EXIT:1
logs=3  ok=3  missing-exit=0  bad-exit=0
```

## The interop answer (the brief's open question)

World-state interop between the two blocks **works at this pin** — no `blocked(...)` was needed:

- the requester reads a crafter's `Target`, `Phase`, `Request` and item slots through
  `Host::block_entity(level, pos)` → `BlockEntityRef::data` / `item_slot`;
- it writes one `Int` (`Request`) into that crafter's saved compound through
  `BlockEntityRef::merge_data`, the documented `/data merge block` path, which loads the merged
  compound into the block entity, so the crafter's own next tick reads it;
- each block otherwise writes only its own compound during its own tick.

Measured in `e2e-launcher.log`: one tick after the filter was set the requester read
`crafter=211,64,0 … requests=2 … stored=8`, and the crafter read `stored=8 crafted=8` — i.e. the
ask crossed, the crafter spent it, and the requester saw the result.

## Coverage (family → evidence)

| family | evidence |
|---|---|
| the machine's band rule (start < min, fill to max, hold) | `runtime::tests::decide_*`; in game: `stored=16 crafted=16` then 60 idle ticks |
| one step = cells out, result in | `a_step_consumes_every_cell_and_makes_the_result`; in game: 4 coal + 4 sticks → 16 torches |
| `not-enough-ingredients` | `a_missing_ingredient_refuses_the_step_and_names_the_cell`; in game: `phase=not-enough-ingredients`, then resume to `crafted=32` |
| `network-full` | `a_full_inventory_refuses_the_step` (**unit only**: reaching 1,728 items in the harness would be 27 stack inserts and proves nothing the unit test does not) |
| the manual step | `a_manual_step_*`; in game: `result=crafted` / `result=not-enough-ingredients` |
| the `active` state | in game: `[active=true]` → `Test passed` while filling, `[active=false]` once idle |
| the container (stock, spill) | in game: `ctl container` loads 8+8; a survival break drops `minecraft:torch:8` + `stepcrafter:step_crafter:1` |
| the recipes (both) | `tools/craft_check.py`: one output each, five ingredients consumed each, wrong pattern crafts nothing |
| the blocks + items (both) | placed and given in game; `stepcrafter:step_crafter_manager` refused by both |
| the commands | every reply parsed: `info`/`target`/`configure`/`requester`/`step`/`take`, and the refusals |
| the tag (`mineable/pickaxe`) | carried for both blocks; the survival break with a diamond pickaxe drops them (a bare hand does not) |
| the requester's rule | `a_requester_*`, `a_batch_caps_each_request`, `a_request_is_not_repeated_while_one_is_outstanding`, `a_failed_task_backs_off_before_asking_again`, `a_zero_batch_filter_is_disabled`, `a_crafter_that_refused_is_the_requesters_failed_task`; in game `requests=2 → stored=8` and `phase=running` at `stored=44` on the 32-step run |
| the interop (requester → crafter) | in game: `stored=8 crafted=8`; the same world without the requester leaves a machine at `stored=0 crafted=0` |
| the cross-block refusal | in game: `phase=not-enough-ingredients backoff=13`, restock → `stored=8 crafted=8` |
| colouring recipes, tags, advancements, managers, monitor, slot upgrade, GUI | `not ported` / `blocked(no-parent-addon-surface)` — `notes.md` §1 |
