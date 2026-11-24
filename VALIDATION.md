# Step Crafter — validation (0.2.9)

Everything here was run on 2026-11-24 against
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice is **the machine's several patterns** (`notes.md` §2j): the machine carries the 45-slot
pattern container from the jar (resource, min, max, batch, status per slot) instead of one target and
one band. Two cells on its panel make two saved slots, and the tick walks them in slot order. The
package is **96,434 B** sha256
`08d5c4773800c0b68f9fb29f599691451bb770455ac2b4e6a59db5c0defba411`, version `0.2.9`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red (unit) | `cargo test --offline` with `Targets::set` stubbed to the old one-target behaviour | `FAILED. 82 passed; 7 failed` (exit 101): `both pattern slots survive` (`left: 1 / right: 2`), `left: Ok(0) / right: Err(TargetsFull)`, `repointing_one_cell_leaves_the_other_target_untouched` (`left: ("minecraft:coal", 0..0)` / `right: ("minecraft:torch", 8..16)`) | `stepcrafter/red-unit-patterns.log` |
| red (game) | `SELFTEST=patterns bash tools/e2e.sh` on this package | `FAIL` + `EXIT:1` — `the machine kept two target slots (targets=2) where today's defect keeps one (targets=1)` | `stepcrafter-e2e/e2e-selftest-patterns.log` |
| unit tests | `cargo test --offline` | **89 passed, 0 failed**, `Compiling stepcrafter v0.2.9` | `stepcrafter/sdk-test.log` |
| format | `rustfmt --edition 2024 --check src/*.rs` | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `Checking stepcrafter v0.2.9`, `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, then `cmp` | `cmp: identical`, sha256 `08d5c477…` twice, **0** `Cargo.lock` entries, 40 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.9 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `96434 bytes, SHA-256 08d5c477…`, `CHECK_EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** (`package matches the tree`, R9 SDK-only, no packed `Cargo.lock`, `tests`/`clippy`/`check`/`fits`/`skills gate` all ok) | `stepcrafter/verify-sdk-package.log` |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=15 ok=15 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`08d5c477…`), the delivered package's.

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-19140d8265a2 built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The old `native-*`
tag carrying this mod was removed before the build, so the bumped manifest forced a real recompile
and no cached build could be selected. Each leg runs through the slot registry with the harness's own
scratch root (`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`).

| run | verdict | log |
|---|---|---|
| main (with the in-run control leg) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control | `FAIL` + `EXIT:1` (`expected stepcrafter:step_crafter at 200 64 0, got minecraft:air`) | `stepcrafter-e2e/e2e-control.log` |
| selftest (requester) | `FAIL` + `EXIT:1` (`the requester reached stored=8 …`; `SELFTEST=crafter` inverts the machine's `stored=16`) | `stepcrafter-e2e/e2e-selftest.log` |
| selftest (no room) | `FAIL` + `EXIT:1` (`the step fit no slot and served nothing …`) | `stepcrafter-e2e/e2e-selftest-capacity.log` |
| selftest (components) | `FAIL` + `EXIT:1` (`the named stack counted no room and the step was refused …`) | `stepcrafter-e2e/e2e-selftest-components-room.log` |
| selftest (take) | `FAIL` + `EXIT:1` (`the taken stacks kept their components …`) | `stepcrafter-e2e/e2e-selftest-take.log` |
| selftest (remainders) | `FAIL` + `EXIT:1` (`the step left the buckets its milk leaves …`) | `stepcrafter-e2e/e2e-selftest-remainder.log` |
| selftest (filters) | `FAIL` + `EXIT:1` (`the requester keeps both filter entries (filters=2) …`) | `stepcrafter-e2e/e2e-selftest-filters.log` |
| selftest (screen) | `FAIL` + `EXIT:1` (`the requester opens its filter screen (container) …`) | `stepcrafter-e2e/e2e-selftest-screen.log` |
| selftest (queue) | `FAIL` + `EXIT:1` (`the second requester was served its own four torches …`) | `stepcrafter-e2e/e2e-selftest-queue.log` |
| selftest (machine panel) | `FAIL` + `EXIT:1` (`the target cell took the carried torch …`) | `stepcrafter-e2e/e2e-selftest-crafterscreen.log` |
| **selftest (several patterns)** | `FAIL` + `EXIT:1` (`the machine kept two target slots (targets=2) where today's defect keeps one (targets=1) …`) | `stepcrafter-e2e/e2e-selftest-patterns.log` |
| red (older package) | the 0.2.6 export run with 0.2.7's leg | `stepcrafter-e2e/e2e-red-queue-0.2.6.log` |
| red (older package) | the 0.2.3 export run with the 0.2.4 leg | `stepcrafter-e2e/e2e-red-components-0.2.3.log` |

## The slice: two target slots, in game

One run of the main leg; every value is parsed from a `ctl` reply, never from the absence of an error:

```
  two cells pressed: stepcrafter 225 64 0 target=minecraft:stick min=- max=- stored=0 phase=idle
    cells=2 result=minecraft:stick:4 crafted=0 … targets=2
    entries=minecraft:torch|-|-|idle|0|0;minecraft:stick|-|-|idle|0|0
  after 80 ticks, the two slots: stepcrafter 225 64 0 target=minecraft:stick min=16 max=32
    stored=32 phase=idle cells=2 result=minecraft:stick:4 crafted=44 …
    stock=minecraft:coal:4,minecraft:oak_planks:2,minecraft:stick:32,minecraft:torch:16
    targets=2
    entries=minecraft:torch|8|16|idle|16|16;minecraft:stick|16|32|idle|28|32
  the machine with only the first cell pressed: … targets=1
    entries=minecraft:torch|8|16|idle|16|16
```

The brief's assertion, read back: two drawn cells on the same panel make `targets=2` with both
entries; `configure-item` gives the torch slot `8..16` and the stick slot `16..32`; with coal, sticks
and oak planks loaded, each slot fills its own band out of the one 27-slot inventory and four coal are
consumed by the torch steps. The second cell is the only difference from the differential machine,
which has one entry and never maintains sticks.

**Differentials / controls:** the machine at the same spot with only the first cell pressed keeps
one slot, the same game without the package places no block at all (`e2e-control.log`), and every
older phase still passes in the same run.

**Fail-capability:** `SELFTEST=patterns` asserts today's one-target reply (`targets=1`) and ends the
run non-zero; the ten older selftests still invert their own assertions and end non-zero.

## Coverage (family → evidence)

`notes.md` §10 is the full table (updated for this slice). The families this slice changed:

| family | evidence |
|---|---|
| the 45-slot pattern container (per-slot target/band/batch/status) | `a_second_target_slot_keeps_its_own_band_beside_the_first`, `the_two_targets_round_trip_through_the_saved_list`, `a_46th_distinct_target_is_refused`, `the_two_targets_round_trip_through_the_saved_compound`; in game `targets=2` with two entries |
| per-slot bands and the tick walking them | `repointing_one_cell_leaves_the_other_target_untouched`, `a_legacy_single_target_compound_still_reads_as_one_slot`, `the_shared_read_is_the_queue_and_the_phase`; in game torch `8..16` and stick `16..32` from one inventory |
| the row of target cells and its open payload | `the_target_cells_payload_is_its_item_id`, `the_machines_cell_and_buttons_are_the_presses_the_server_applies`, `a_machine_press_round_trips_through_its_button_id`; in game cell 1 adds the stick slot |

## Skills

`Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-review,rust-port`
(boot: `AGENTS.md`, `ARCHITECTURE.md` §3–§10, `ports/stepcrafter/{notes,VALIDATION,FINDINGS}.md`,
`horizon/docs/{modpacks,mods,native-mods}.md`, `ports/_shared/AGENT_MOD_STRUCTURE.md` §0,
`ports/_shared/build-slots-usage.md`, the jar at `ports/_shared/licence-cache/`, the SDK at
`ports/_shared/sdk/0a875da/`; `CONTENT.md` is not present in the port — its content is `assets/`+
`data/` and §9 of `notes.md`).
