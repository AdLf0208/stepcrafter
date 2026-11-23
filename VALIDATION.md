# Step Crafter — validation (0.2.8)

Everything here is a command that was run on 2026-11-23, with its artefact under
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice is **the Step Crafter's own configuration screen** (`notes.md` §2i): the machine was
configured only by chat while the requester had its own panel. The machine now opens its own menu
type — the same 27 stock slots as real menu slots, plus a drawn target cell and the band's four
buttons — and both the cell and the buttons write the same saved compound `/stepcrafter
target|configure` writes. The package is **90,176 B** sha256
`60f93e175ec3122f602058f5bdd851f90438cb2a57683d1bb902727878b17010`, version `0.2.8`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red (unit) | `cargo test --offline` with `MachineEditor`, `machine_data_slots` and the cell payload as stubs | `FAILED. 77 passed; 6 failed` (exit 101): `a carried torch must make the machine's target`, `assertion failed: editor.apply(MachinePress::MaxUp, None)`, `the screen's min and max` (`left: [0, 0] / right: [8, 16]`), `left: "" / right: "minecraft:torch"`, and the two other rule tests | `stepcrafter/red-unit-crafterscreen.log` |
| red (game) | `SELFTEST=crafterscreen bash tools/e2e.sh` on this package | `FAIL` + `EXIT:1` — `the target cell took the carried torch where today's panel has no cell and sets no target` (today's defect stated as a run) | `stepcrafter-e2e/e2e-selftest-crafterscreen.log` |
| unit tests | `cargo test --offline` | **83 passed, 0 failed**, `Compiling stepcrafter v0.2.8` | `stepcrafter/sdk-test.log` |
| format | `rustfmt --edition 2024 --check src/*.rs` on the port's eight sources (not `cargo fmt --all`: it would reach into `.horizon/sdk`) | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `Checking stepcrafter v0.2.8`, `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, then `cmp` | `cmp: identical`, sha256 `60f93e17…` twice, **0** `Cargo.lock` entries, 40 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.8 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `90176 bytes, SHA-256 60f93e17…`, `CHECK_EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** (`package matches the tree`, R9 SDK-only, no packed `Cargo.lock`, `tests`/`clippy`/`check`/`fits`/`skills gate` all ok) | `stepcrafter/verify-sdk-package.log` |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=13 ok=13 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`60f93e17…`), the delivered package's.

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/build-slot.sh run --job W1-stepcrafter --port ports/stepcrafter --step build --
ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-4c6fc872dead built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The version bump made
it recompile, so the exported binary is the delivered sources, not a reused manifest; no `native-*`
tag carrying this mod existed before the build, so none could be selected in its place.

Each run is one command through the slot registry, with the harness's own scratch root
(`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`). Two runs of the same port are
**not** concurrent: the e2e guard keys its ctl shim by mod, so a parallel run deletes the other's.

| run | verdict | log |
|---|---|---|
| main (with the in-run control leg) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control | `FAIL` + `EXIT:1` (`expected stepcrafter:step_crafter at 200 64 0, got minecraft:air`) | `stepcrafter-e2e/e2e-control.log` |
| selftest (requester) | `FAIL` + `EXIT:1` (`the requester reached stored=8 …`; `SELFTEST=crafter` inverts the machine's `stored=16` instead) | `stepcrafter-e2e/e2e-selftest.log` |
| selftest (no room) | `FAIL` + `EXIT:1` (`the step fit no slot and served nothing …`) | `stepcrafter-e2e/e2e-selftest-capacity.log` |
| selftest (components) | `FAIL` + `EXIT:1` (`the named stack counted no room and the step was refused …`) | `stepcrafter-e2e/e2e-selftest-components-room.log` |
| selftest (take) | `FAIL` + `EXIT:1` (`the taken stacks kept their components …`) | `stepcrafter-e2e/e2e-selftest-take.log` |
| selftest (remainders) | `FAIL` + `EXIT:1` (`the step left the buckets its milk leaves …`) | `stepcrafter-e2e/e2e-selftest-remainder.log` |
| selftest (filters) | `FAIL` + `EXIT:1` (`the requester keeps both filter entries (filters=2) …`) | `stepcrafter-e2e/e2e-selftest-filters.log` |
| selftest (screen) | `FAIL` + `EXIT:1` (`the requester opens its filter screen (container) …`) | `stepcrafter-e2e/e2e-selftest-screen.log` |
| selftest (queue) | `FAIL` + `EXIT:1` (`the second requester was served its own four torches …`) | `stepcrafter-e2e/e2e-selftest-queue.log` |
| **selftest (machine panel)** | `FAIL` + `EXIT:1` (`the target cell took the carried torch where today's panel has no cell …`) | `stepcrafter-e2e/e2e-selftest-crafterscreen.log` |
| red (previous package) | the 0.2.6 export run with 0.2.7's leg | `stepcrafter-e2e/e2e-red-queue-0.2.6.log` |
| red (older package) | the 0.2.3 export run with the 0.2.4 leg | `stepcrafter-e2e/e2e-red-components-0.2.3.log` |

The eight earlier selftests and the two historical red runs are the same package's and the same
script's as before this slice, except for one harness line added while proving `SELFTEST=screen`: the
kill box at the re-place spot now clears mobs as well as item entities (an Enderman that wandered
onto the spot refused the placement). The runs whose scenario passes that phase — main, take,
remainder, screen, queue, machine panel — were **re-run with the final script**; the control leg and
the selftests that fail earlier (`1`, `crafter`, `capacity`, `components-room`, `filters`) cannot
reach the edit.

```
$ python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict
ok       stepcrafter-e2e        e2e-control.log            FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-launcher.log           PASS + EXIT:0
ok       stepcrafter-e2e        e2e-red-components-0.2.3.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-red-queue-0.2.6.log    FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-capacity.log  FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-components-room.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-crafterscreen.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-filters.log   FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-queue.log     FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-remainder.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-screen.log    FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-take.log      FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest.log           FAIL + EXIT:1
logs=13  ok=13  missing-exit=0  bad-exit=0
```

## The slice: the machine's own panel, in game

One run of the main leg (the phase after the requester's screen); every value is parsed from a `ctl`
reply, never from the absence of an error:

```
  the fresh machine: stepcrafter 225 64 0 target=none min=- max=- stored=0 phase=idle cells=0
    result=none crafted=0 queued=0 asks=none served=none items=0 stock=empty
  the machine's panel: container, 27 stock slots, top 16
  after the torch onto the target cell: stepcrafter 225 64 0 target=minecraft:torch min=- max=-
    stored=0 phase=idle cells=2 result=minecraft:torch:4 crafted=0 queued=0 asks=none served=none
    items=0 stock=empty
  the band the panel's buttons set: stepcrafter 225 64 0 target=minecraft:torch min=8 max=16
    stored=0 phase=not-enough-ingredients cells=2 result=minecraft:torch:4 …
  the band's guard (max - to 8..8, then min +): … min=8 max=8 stored=0 …
  the machine nobody opened: stepcrafter 225 64 4 target=none min=- max=- stored=0 phase=idle …
```

The brief's assertion, read back: the right click opens `container` with **27** stock slots (the
menu's own slots, at 300..326), the panel's top is **16** against the chest panel's 37 — a centred
208-high panel — a carried `minecraft:torch` on the drawn target cell makes the machine maintain it
and resolve `cells=2 result=minecraft:torch:4`, `max +` ×16 then `min +` ×8 read `min=8 max=16`, and
`max -` ×8 followed by a `min +` leaves `min=8 max=8` (the inversion is refused). The screenshot
`screen-crafter.png` is the open panel.

**Differentials / controls:** the machine at `225 64 4`, never opened, reads `target=none min=- max=-`
— the panel presses are the only difference between the two; the same game **without** the package
places no block at all and has no `stepcrafter` command (`e2e-control.log`); and every older phase of
the scenario (band rule, room, components, remainders, take, both screens, filters, queue) still
passes in the same run.

**Fail-capability:** `SELFTEST=crafterscreen` asserts today's behaviour (a panel with no target cell,
so the carried torch clicked on it changes nothing and the machine still reads `target=none`) and
ends the run non-zero; the eight older selftests still invert their own assertions and still end
non-zero.

## Coverage (family → evidence)

`notes.md` §10 is the full table (updated for this slice). The families this slice changed:

| family | evidence |
|---|---|
| the machine's own menu and panel (open, 27 stock slots, size) | `the_machines_stock_slots_are_its_containers_twenty_seven`, `the_machines_menu_declares_the_slots_the_screen_publishes`, `the_machines_cell_and_buttons_are_the_presses_the_server_applies`; in game `container`, 27 slots, top 16 |
| the target cell sets the machine's target and resolves its pattern | `the_target_cell_takes_the_carried_item`, `the_target_cell_with_the_same_item_changes_nothing`; in game `target=minecraft:torch cells=2 result=minecraft:torch:4` |
| the band's buttons, and a step that would invert the band | `the_value_buttons_step_the_band_and_refuse_an_inversion`, `a_machine_value_never_goes_below_zero_and_caps_at_the_screens_maximum`; in game `min=8 max=16`, then `8..8` kept by a refused `min +` |
| the numbers and the cell's payload are 16-bit/wire-safe | `the_machines_data_slots_are_its_band`, `the_target_cells_payload_is_its_item_id` |
| the panel and a machine nobody opened | in game the same world's second machine reads `target=none min=- max=-` |

## Skills

`Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices`
(boot: `AGENTS.md`, `ARCHITECTURE.md` §3–§10, `ports/stepcrafter/{notes,VALIDATION,FINDINGS}.md`,
`horizon/docs/{modpacks,mods,native-mods}.md`, `ports/_shared/AGENT_MOD_STRUCTURE.md` §0,
`ports/_shared/build-slots-usage.md`, the SDK at `ports/_shared/sdk/0a875da/`; `CONTENT.md` is not
present in the port — its content is `assets/`+`data/` and §9 of `notes.md`).
