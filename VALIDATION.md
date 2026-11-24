# Step Crafter — validation (0.2.10)

Everything here was run on 2026-11-24 against
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice is **the full 45-cell pattern grid** (`notes.md` §2k): the machine panel grows from nine
drawn cells to upstream's whole 9 × 5 grid, so cell 44 can be pressed to make the 45th saved pattern
slot. It is a screen-layout change over the saved `Targets` container 0.2.9 carried, plus the button
decoder that lets the 45th cell reach the server. The package is **97,388 B** sha256
`9bb9770bb572b52118df5fd9c80f68096d34b555adb66976c29298e8096c0760`, version `0.2.10`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red (unit) | `cargo test --offline` with the old nine-cell panel and the button decoder stopping at 340 | `FAILED. 90 passed; 3 failed` (exit 101): `the_forty_fifth_cell_button_reaches_the_editor` (`left: None` / `right: Some(Cell(44))`), `the_forty_fifth_cell_is_on_the_machine_panel` (`left: None` / `right: Some(344)`), `the_machine_panel_is_the_requesters_height` (`left: (176.0, 208.0)` / `right: (176.0, 238.0)`) | `stepcrafter/red-unit-grid45.log` |
| red (game) | `SELFTEST=grid45 bash tools/e2e.sh` on this package | `FAIL` + `EXIT:1` — the old nine-cell reply has no 45th slot | `stepcrafter-e2e/e2e-selftest-grid45.log` |
| unit tests | `cargo test --offline` | **94 passed, 0 failed**, `Compiling stepcrafter v0.2.10` | `stepcrafter/sdk-test.log` |
| format | `rustfmt --edition 2024 --check src/*.rs` | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `Checking stepcrafter v0.2.10`, `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, then `cmp` | `cmp: identical`, sha256 `9bb9770b…` twice, **0** `Cargo.lock` entries, 40 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.10 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `97388 bytes, SHA-256 9bb9770b…`, `CHECK_EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** (`package matches the tree`, R9 SDK-only, no packed `Cargo.lock`, `tests`/`clippy`/`check`/`fits`/`skills gate` all ok) | `stepcrafter/verify-sdk-package.log` |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=16 ok=16 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`9bb9770b…`), the delivered package's.

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-b63be1e5dfb0 built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The stale `native-*`
tag carrying 0.2.9 was removed before the build, so the bumped manifest forced a real recompile and
no cached build could be selected. Each leg runs through the slot registry with the harness's own
scratch root and a per-leg socket (`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`).
The launcher's build and every game leg are local: that machine alone has the launcher instance and
the exported game binaries. All Rust steps (`test`, `lint`, `build`, `check`) forwarded to
`AdLf-PC` through `build-slot.sh`.

| run | verdict | log |
|---|---|---|
| main (with the in-run control leg) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control (the Nightly without the package as `GAME`) | `FAIL` + `EXIT:1` (`expected stepcrafter:step_crafter at 200 64 0, got minecraft:air`) | `stepcrafter-e2e/e2e-control.log` |
| selftest (requester) | `FAIL` + `EXIT:1` (`the requester reached stored=8 …`; `SELFTEST=crafter` inverts the machine's `stored=16`) | `stepcrafter-e2e/e2e-selftest.log` |
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
| **selftest (full 45-cell grid)** | `FAIL` + `EXIT:1` (`cell 44 made a 45th slot where today's nine-cell panel changes nothing …`) | `stepcrafter-e2e/e2e-selftest-grid45.log` |
| red (older package) | the 0.2.6 export run with 0.2.7's leg | `stepcrafter-e2e/e2e-red-queue-0.2.6.log` |
| red (older package) | the 0.2.3 export run with 0.2.4's leg | `stepcrafter-e2e/e2e-red-components-0.2.3.log` |

Two legs (`components-room`, `queue`) first failed on an empty `ctl messages` reply while three game
processes shared the machine — a harness flake, not a port verdict (the same empty-reply class is
recorded in the port's notes). Each was re-run alone and ended at its intended `SELFTEST` assertion
with `EXIT:1`; the final logs above are those re-runs.

## The slice: the 45th cell, in game

One run of the main leg; every value is parsed from a `ctl` reply, never from "no error":

```
  before the press: stepcrafter 225 64 0 … targets=44 entries=minecraft:stone|-|-|idle|0|0;…;minecraft:crimson_planks|-|-|idle|0|0
  after cell 44: stepcrafter 225 64 0 target=minecraft:torch min=- max=- stored=0 phase=idle
    cells=2 result=minecraft:torch:4 crafted=0 … targets=45
    entries=minecraft:stone|-|-|idle|0|0;…;minecraft:crimson_planks|-|-|idle|0|0;minecraft:torch|-|-|idle|0|0
  targets and entries: targets=45, 45 entry/ies
  the machine nobody opened: stepcrafter 225 64 4 … targets=44 entries=minecraft:stone|…;…;minecraft:crimson_planks|-|-|idle|0|0
```

The brief's assertion, read back: both machines are planted with the same 44 saved slots; the panel
is read back from the game as the centred 176 × 238 one; cell 44 at its computed local pixel
`(135, 118)` with a carried torch makes `targets=45` and a **45th** `entries=` field at index 44
(`minecraft:torch|-|-|idle|0|0`). The screenshot `screen-grid45.png` shows the open grid; the parsed
reply is the assertion. The differential machine, identically planted but never opened, keeps
`targets=44` with exactly 44 entries — the press is the only difference.

**Fail-capability:** `SELFTEST=grid45` asserts today's no-45th-slot reply and ends the run non-zero;
the other eleven selftests and the no-package control still invert their own assertions and end
non-zero in the same package.

## Coverage (family → evidence)

`notes.md` §10 is the full table (updated for this slice). The family this slice changed:

| family | evidence |
|---|---|
| the full 9 × 5 pattern grid on the machine panel | `the_forty_fifth_cell_is_on_the_machine_panel`, `every_one_of_the_forty_five_cells_presses_its_own_slot`, `the_machine_panel_is_the_requesters_height`, `the_forty_fifth_cell_button_reaches_the_editor`, `the_forty_fifth_cell_makes_the_forty_fifth_slot`; in game cell 44's pixel makes `targets=45` with 45 entries and the torch at index 44, while the machine nobody opened stays at 44; `SELFTEST=grid45` ends non-zero |

## Skills

`Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-review,rust-port`
(boot: `AGENTS.md`, `ARCHITECTURE.md` §3–§10, `ports/stepcrafter/{notes,VALIDATION,FINDINGS}.md`,
`horizon/docs/{modpacks,mods,native-mods}.md`, `ports/_shared/AGENT_MOD_STRUCTURE.md` §0,
`ports/_shared/build-slots-usage.md`, the jar at `ports/_shared/licence-cache/`, the SDK at
`ports/_shared/sdk/0a875da/`; `CONTENT.md` is not present in the port — its content is `assets/`+
`data/` and §9 of `notes.md`).
