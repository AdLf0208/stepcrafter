# Step Crafter — validation (0.2.0)

Everything here is a command that was run on 2026-11-21, with its artefact under
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red | `cargo test --offline` with the rule stubbed | `FAILED. 4 passed; 8 failed` (exit 101) | `stepcrafter/red-unit.log` |
| unit tests | `cargo test --offline` | **17 passed, 0 failed** | `stepcrafter/sdk-test.log` |
| format | `cargo fmt --check` | `FMT-CLEAN` (exit 0) | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | 0 warnings (exit 0) | `stepcrafter/sdk-clippy.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `native-mod, format native-1/0a875da…/e59da90…, for 26.3`, `33589 bytes, SHA-256 32b80ec2…`, `EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits` | `stepcrafter/sdk-fits-0b14f8a.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice | `cmp` identical, same sha256, **0** `Cargo.lock` entries | `stepcrafter/sdk-build-twice.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** | this file's §gate |

Every log names the sha it ran against (`32b80ec2…`).

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)`
→ `native-0a875da-c0bfa18cc45c built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`; it recompiled, so the
exported binary is the delivered sources, not a reused manifest). The three runs below are against
that export. The three runs are one command each,
all through the slot registry (`BUILD_LOCAL=1 ports/tools/build-slot.sh run --job W1-stepcrafter
--port ports/stepcrafter --step e2e -- …`):

| run | command | verdict | log |
|---|---|---|---|
| main | `bash tools/e2e.sh` (`GAME` = the built game, `CONTROL_GAME` = the same Nightly without the package) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control | `GAME=<control game> bash tools/e2e.sh` | `FAIL` + `EXIT:1` (the block id is unknown) | `stepcrafter-e2e/e2e-control.log` |
| selftest | `SELFTEST=1 bash tools/e2e.sh` | `FAIL` + `EXIT:1` (the inverted headline assertion) | `stepcrafter-e2e/e2e-selftest.log` |

```
$ python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict
ok       stepcrafter-e2e        e2e-control.log            FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-launcher.log           PASS + EXIT:0
ok       stepcrafter-e2e        e2e-selftest.log           FAIL + EXIT:1
logs=3  ok=3  missing-exit=0  bad-exit=0
```

## Coverage (family → evidence)

| family | evidence |
|---|---|
| the machine's band rule (start < min, fill to max, hold) | `runtime::tests::decide_*`; in game: `stored=16 crafted=16` then 60 idle ticks |
| one step = cells out, result in | `a_step_consumes_every_cell_and_makes_the_result`; in game: 4 coal + 4 sticks → 16 torches |
| `not-enough-ingredients` | `a_missing_ingredient_refuses_the_step_and_names_the_cell`; in game: `phase=not-enough-ingredients`, then resume to `crafted=32` |
| `network-full` | `a_full_inventory_refuses_the_step` (**unit only**: reaching 1,728 items in the harness would be 27 stack inserts and proves nothing the unit test does not) |
| the manual step | `a_manual_step_*`; in game: `result=crafted` / `result=not-enough-ingredients` |
| the `active` state | in game: `execute if block … [active=true]` → `Test passed` while filling, `[active=false]` once idle |
| the container (stock, spill) | in game: `ctl container` loads 8+8; a survival break drops `minecraft:torch:8` + `stepcrafter:step_crafter:1` |
| the recipe | `tools/craft_check.py`: one item out, five ingredients consumed, wrong pattern crafts nothing |
| the block + item | placed and given in game; `stepcrafter:step_crafter_manager` refused by both |
| the commands | every reply parsed: `info`/`target`/`configure`/`step`/`take`, and the refusals |
| the tag (`mineable/pickaxe`) | carried; the survival break with a diamond pickaxe drops the block (a bare hand does not) |
| colouring recipes, tags, advancements, requester, managers, monitor, slot upgrade, GUI | `not ported` / `blocked(no-parent-addon-surface)` — `notes.md` §1 |
