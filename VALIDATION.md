# Step Crafter — validation (0.2.11)

Everything here was run on 2026-11-24 against
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice is **the block's `direction` state** (`notes.md` §2l): the six-value `active` × `direction`
blockstate, the jar's per-face model rotations, and upstream's `FACE_CLICKED` placement rule
(clicked face, not yaw). The package is **99,600 B** sha256
`31a87f798a6282663c0b4b74c6fa2c5241f12cd5a1308d93e21acc16559de377`, version `0.2.11`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red (unit) | `cargo test --offline` with `direction_for_face` a stub answering `None` | `FAILED. 95 passed; 1 failed` (exit 101): `a_clicked_face_becomes_the_blocks_direction` (`left: None` / `right: Some("down")`) | `stepcrafter/red-unit-direction.log` |
| red (game) | `SELFTEST=direction bash tools/e2e.sh` on this package | `FAIL` + `EXIT:1` — "the machine has a direction state and the run must end non-zero" | `stepcrafter-e2e/e2e-selftest-direction.log` |
| unit tests | `cargo test --offline` | **96 passed, 0 failed**, `Compiling stepcrafter v0.2.11` | `stepcrafter/sdk-test.log` |
| format | `cargo fmt --check` | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `Checking stepcrafter v0.2.11`, `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, then `cmp` | `cmp: identical`, sha256 `31a87f79…` twice, **0** `Cargo.lock` entries, 40 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.11 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `99600 bytes, SHA-256 31a87f79…`, `CHECK_EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** (`package matches the tree`, R9 SDK-only, no packed `Cargo.lock`, `tests`/`clippy`/`check`/`fits`/`skills gate` all ok) | `stepcrafter/verify-sdk-package.log` |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=17 ok=17 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`31a87f79…`), the delivered package's.

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-45d15e986c7d built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The stale `native-*`
tag carrying 0.2.10 was removed before each build, so the bumped manifest forced a real recompile
and no cached build could be selected. Each leg runs through the slot registry with the harness's own
scratch root and a per-leg socket (`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`).
The launcher's build and every game leg are local: that machine alone has the launcher instance and
the exported game binaries. All Rust steps (`test`, `lint`, `build`, `check`) forwarded to
`AdLf-PC` through `build-slot.sh`.

A load-time trap was found and fixed before delivery: attaching `StepCrafter` as both a block-entity
Part and a block Part made the launcher refuse the game with
`stepcrafter::StepCrafter is attached to stepcrafter:step_crafter twice`. The placement hook now
lives on a dedicated `StepCrafterPlacement` block Part; the package was rebuilt and the ladder
re-run from scratch (the log above is the final run).

| run | verdict | log |
|---|---|---|
| main | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control (the Nightly without the package as `GAME`) | `FAIL` + `EXIT:1` (`Unknown block type 'stepcrafter:step_crafter'`) | `stepcrafter-e2e/e2e-control.log` |
| selftest (requester) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest.log` |
| selftest (machine fill) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest-crafter.log` |
| selftest (no room) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest-capacity.log` |
| selftest (components) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest-components-room.log` |
| selftest (take) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest-take.log` |
| selftest (remainders) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest-remainder.log` |
| selftest (filters) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest-filters.log` |
| selftest (screen) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest-screen.log` |
| selftest (queue) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest-queue.log` |
| selftest (machine panel) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest-crafterscreen.log` |
| selftest (several patterns) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-selftest-patterns.log` |
| selftest (full 45-cell grid) | `FAIL` + `EXIT:1` | `stepcrafter-e2e/e2e-grid45.log` |
| **selftest (direction)** | `FAIL` + `EXIT:1` (`SELFTEST=direction: the machine has a direction state …`) | `stepcrafter-e2e/e2e-selftest-direction.log` |
| red (older package) | the 0.2.6 export run with 0.2.7's leg | `stepcrafter-e2e/e2e-red-queue-0.2.6.log` |
| red (older package) | the 0.2.3 export run with 0.2.4's leg | `stepcrafter-e2e/e2e-red-components-0.2.3.log` |

The main leg no longer launches the control game in-process (two launches in one leg proved flaky
under concurrent agent load); the control is its own log above. Several legs needed a re-run when a
shared box delayed a launch or emptied a `ctl messages` reply — a harness flake, not a port verdict;
the final logs are those that reached their intended assertion.

## The slice: `direction`, in game

One run of the main leg; every value is parsed from a `ctl` reply, never from "no error":

```
  place on the north face: {"block":{"block":"stepcrafter:step_crafter","pos":[200,66,-1],
    "state":"stepcrafter:step_crafter[active=false,direction=north]"} ...}
  north placement, [direction=north]: Test passed
  place on the south face: {"block":{"block":"stepcrafter:step_crafter","pos":[200,66,1],
    "state":"stepcrafter:step_crafter[active=false,direction=south]"} ...}
  south placement, [direction=south]: Test passed
  south placement, [direction=north]: Test failed
  vanilla stone, [direction=north]: Test failed
```

The brief's assertion, read back: a floating support at `200 66 0` is approached from the north
(player `200.5 64 -3.5`, `ctl look-at-block 200 66 0 north`) and `ctl place 200 66 -1` makes a
machine whose `execute if block 200 66 -1 stepcrafter:step_crafter[direction=north]` answers
`Test passed`. From the south, the same support yields `direction=south`: that predicate passes and
the north one fails — the clicked face, not the placer's yaw. Vanilla stone at `202 66 0` named
with `[direction=north]` passes neither predicate, so the state is the port's, not a harness echo.

**Fail-capability:** `SELFTEST=direction` asserts the old no-state reply and ends the run non-zero
at the north predicate; the other selftests and the no-package control still invert their own
assertions and end non-zero in the same package.

## Coverage (family → evidence)

`notes.md` §10 is the full table (updated for this slice). The family this slice changed:

| family | evidence |
|---|---|
| the six-value `direction` state and the `FACE_CLICKED` placement rule | `a_clicked_face_becomes_the_blocks_direction`, `an_unknown_face_is_not_a_direction`; in game a north-face placement reads `[active=false,direction=north]` and passes `[direction=north]`, a south-face placement passes `[direction=south]` and fails `[direction=north]`, vanilla stone fails both; `SELFTEST=direction` ends non-zero |

## Skills

`Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-review,rust-port`
(boot: `AGENTS.md`, `ARCHITECTURE.md` §3–§10, `ports/stepcrafter/{notes,VALIDATION,FINDINGS}.md`,
`horizon/docs/{modpacks,mods,native-mods}.md`, `ports/_shared/AGENT_MOD_STRUCTURE.md` §0,
`ports/_shared/build-slots-usage.md`, the jar at `ports/_shared/licence-cache/`, the SDK at
`ports/_shared/sdk/0a875da/`; `CONTENT.md` is not present in the port — its content is `assets/`+
`data/` and §9 of `notes.md`).
