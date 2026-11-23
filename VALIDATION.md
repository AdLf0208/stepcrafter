# Step Crafter — validation (0.2.6)

Everything here is a command that was run on 2026-11-23, with its artefact under
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice is **the Step Requester's filter screen** (`notes.md` §2g): the 45 entries upstream edits in
its GUI, as 45 drawn cells on a 176 × 238 panel with the focused entry's `min`/`max`/`batch` and the
buttons that step them, plus the mod's own menu open for both blocks (the engine opens a mod
container's menu for nobody, §2g trap 1). The package is **78,612 B** sha256
`6463ca1d05de67a3a2d6d890a76658b672e98aad82b655ff7b65479c4ea251b9`, version `0.2.6`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red (unit) | `cargo test --offline` with `Editor`/`Press`/`view_data_slots`/`extra_items` as stubs | `FAILED. 49 passed; 11 failed` (exit 101): `assertion failed: editor.apply(Press::Cell(0), Some("minecraft:torch"))`, `… Press::MinUp …`, `… Press::Remove …`, `left: [0, 0, 0, 0, 0] / right: [2, 1, 8, 16, 8]` | `stepcrafter/red-unit-screen.log` |
| red (game) | `SELFTEST=screen bash tools/e2e.sh` | `FAIL: SELFTEST=screen: the requester opens its filter screen (container) where today's defect opens none (playing), so the run must end non-zero` | `stepcrafter-e2e/e2e-selftest-screen.log` |
| measured route | a throwaway probe (a scratch script, deleted): a right click on both blocks with an empty hand and with an item, before and after the mod's own `open_menu` | before: `screen: {"screen":"playing","slots":0}` twice; after: `container`, 63 slots (the machine) and 36 (the requester) | `stepcrafter/probe-menu.log` |
| unit tests | `cargo test --offline` | **64 passed, 0 failed** | `stepcrafter/sdk-test.log` |
| format | `cargo fmt --check` on the port's own sources (not `--all`: it would reach into `.horizon/sdk`) | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, copying `target/horizon-mod/stepcrafter.hmod` | `cmp` identical, sha256 `6463ca1d…` twice, **0** `Cargo.lock` entries, 39 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.6 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `78612 bytes, SHA-256 6463ca1d…`, `CHECK_EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** (also `package matches the tree`) | `stepcrafter/verify-sdk-package.log` |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=10 ok=10 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`6463ca1d…`).

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/build-slot.sh run --job W1-stepcrafter --port ports/stepcrafter --step build --
ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-055b78db0f78 built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The version bump made
it recompile, so the exported binary is the delivered sources, not a reused manifest.

Each run is one command, through the slot registry, with the harness's own scratch root
(`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`). Two runs of the same port are
**not** concurrent: the e2e guard keys its ctl shim by mod, so a parallel run deletes the other's
(measured: the concurrent `SELFTEST=screen` leg died with `arena chunk never loaded`).

| run | command | verdict | log |
|---|---|---|---|
| main | `launcher-game.sh run stepcrafter` (`GAME` = the built game, in-run `CONTROL_GAME` leg) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control | `GAME=$CONTROL_GAME CONTROL_GAME= bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`expected stepcrafter:step_crafter at 200 64 0, got minecraft:air`) | `stepcrafter-e2e/e2e-control.log` |
| selftest (requester) | `SELFTEST=1 bash ports/stepcrafter/tools/e2e.sh` | `FAIL` + `EXIT:1` (`the requester reached stored=8 …`) | `stepcrafter-e2e/e2e-selftest.log` |
| selftest (no room) | `SELFTEST=capacity …` | `FAIL` + `EXIT:1` (`the step fit no slot and served nothing …`) | `stepcrafter-e2e/e2e-selftest-capacity.log` |
| selftest (components) | `SELFTEST=components-room …` | `FAIL` + `EXIT:1` (`the named stack counted no room and the step was refused …`) | `stepcrafter-e2e/e2e-selftest-components-room.log` |
| selftest (take) | `SELFTEST=take …` | `FAIL` + `EXIT:1` (`the taken stacks kept their components …`) | `stepcrafter-e2e/e2e-selftest-take.log` |
| selftest (remainders) | `SELFTEST=remainder …` | `FAIL` + `EXIT:1` (`the step left the buckets its milk leaves …`) | `stepcrafter-e2e/e2e-selftest-remainder.log` |
| selftest (filters) | `SELFTEST=filters …` | `FAIL` + `EXIT:1` (`the requester keeps both filter entries (filters=2) …`) | `stepcrafter-e2e/e2e-selftest-filters.log` |
| **selftest (screen)** | `SELFTEST=screen …` | `FAIL` + `EXIT:1` (`the requester opens its filter screen (container) where today's defect opens none (playing) …`) | `stepcrafter-e2e/e2e-selftest-screen.log` |
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
ok       stepcrafter-e2e        e2e-selftest-screen.log    FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-take.log      FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest.log           FAIL + EXIT:1
logs=10  ok=10  missing-exit=0  bad-exit=0
```

## The slice: the filter screen, in game

Everything is one run of the main leg (phase 23 of `tools/e2e.sh`); the values are parsed from
`ctl screen`/`ctl ui`/`ctl messages`, never from the absence of an error:

```
  the machine's stock menu: container, 27 container slots, top 37
  the screen opened: container, slots 36
  the screen: container; after the first cell's click:
    stepcrafter 223 64 0 kind=requester filters=1 item=minecraft:torch min=0 max=0 batch=0 …
  the band guard (min + at 0..0, then max + and -):
    … filters=1 item=minecraft:torch min=0 max=0 batch=0 …
  the entry the screen made:
    … filters=1 item=minecraft:torch min=4 max=8 batch=4 …
  after a stick onto the torch's cell:
    … filters=1 item=minecraft:stick min=4 max=8 batch=4 …
  the command's entry beside the screen's:
    stepcrafter requester 223 64 0 item=minecraft:torch min=4 max=8 batch=4 filters=2
  after the screen's remove button:
    … filters=1 item=minecraft:stick min=4 max=8 batch=4 …
  the unopened requester beside it:
    … filters=0 item=none min=- max=- batch=- …
```

The panel is checked from the game itself: the requester's top is **36** above the machine's 166-high
chest panel (the declared `(238 − 166) / 2`), its left is the centred 176, and the player's hotbar
slot is where the layout declares it. `screen-requester.png` is the open screen (title, 45 cells with
the torch's icon, `min 4`, `max 8`, `batch 4`, `remove`).

**Differentials / controls:** the requester one row over, never opened, carries no entry
(`filters=0`), so the click is the only difference; the machine's stock menu opens 27 slots while the
requester's opens none of its own (its single container slot is hidden by the layout); a `min +` at
0..0 is refused (the band cannot invert); and the whole scenario runs on the Nightly **without** the
package (`e2e-control.log`), where the block is air.

**Fail-capability:** `SELFTEST=screen` asserts today's defect (`screen: playing`, no entry from a
menu) and ends the run non-zero on this package; the six older selftests still invert their own
assertions and still end non-zero.

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
| the `active` states | in game: `[active=true]` → `Test passed` while filling, `[active=false]` once idle, on both blocks |
| the container (stock, spill) | in game: `ctl container` loads 8+8; a survival break drops `minecraft:torch:8` + `stepcrafter:step_crafter:1` |
| the recipes (both) | `tools/craft_check.py`: one output each, five ingredients consumed each, wrong pattern crafts nothing |
| the blocks + items (both) | placed and given in game; `stepcrafter:step_crafter_manager` refused by both |
| the commands | every reply parsed: `info`/`target`/`configure`/`requester add|remove`/`step`/`take`, and the refusals |
| `take` (the stored stacks, components and damage) | in game: Alpha/Beta named stacks, a Blade sword with `damage 7` and `sharpness 3`; `SELFTEST=take` ends non-zero |
| the tag (`mineable/pickaxe`) | carried for both blocks; the survival break with a diamond pickaxe drops them (a bare hand does not) |
| the requester's rule | `a_requester_*`, `a_batch_caps_each_request`, `a_request_is_not_repeated_while_one_is_outstanding`, `a_failed_task_backs_off_before_asking_again`, `a_zero_batch_filter_is_disabled`, `a_crafter_that_refused_is_the_requesters_failed_task`; in game `requests=2 → stored=8` and `phase=running` at `stored=44` on the 32-step run |
| several filter entries, each served by its own crafter | `a_second_filter_entry_is_kept_beside_the_first`, `a_filter_entry_is_replaced_by_its_own_resource`, `a_requester_carries_at_most_the_containers_45_entries`, `a_removed_entry_leaves_the_others`, `a_filter_entry_round_trips_through_its_saved_form`, `the_filter_container_round_trips_through_its_saved_form`, `a_legacy_single_filter_compound_still_reads_as_one_entry`; in game: `filters=2` with both entries and their stored counts, `crafted=8` + `crafted=16`, while one entry in the same world leaves the stick crafter at `crafted=0`; `SELFTEST=filters` ends non-zero |
| **the requester's filter screen** | `a_press_round_trips_through_its_button_id`, `a_click_on_an_empty_cell_with_a_carried_item_adds_the_entry`, `a_click_on_a_filled_cell_selects_it`, `a_click_with_another_item_repoints_the_entry_and_keeps_its_band`, `a_click_that_would_give_two_entries_one_resource_is_refused`, `a_step_keeps_the_band_valid`, `a_step_never_goes_below_zero_and_a_zero_batch_stays_disabled`, `a_batch_step_enables_an_entry`, `a_remove_press_drops_the_selected_entry_and_keeps_the_others`, `a_cell_beyond_the_next_free_one_changes_nothing`, `a_step_on_an_empty_requester_changes_nothing`, `the_data_slots_are_the_selected_entry`, `the_screen_payload_lists_every_entrys_resource`, `the_editor_and_the_command_are_one_container`, `the_grid_is_nine_by_five_and_the_cells_do_not_overlap`, `the_buttons_are_the_presses_the_server_applies`, `the_panel_holds_its_rows_and_the_inventory`, `the_menu_declares_the_slots_the_screen_publishes`; in game: the screen opens, a carried torch makes entry 0, the buttons set `4 8 4`, a carried stick repoints the cell keeping the band, the command's entry is a cell the remove button drops, and an unopened requester has none; `SELFTEST=screen` ends non-zero |
| the screen's panel and origin | in game: the top is 36 above the machine's 166-high panel (the declared 238), the left is the centred 176, `slot-0` is the hotbar |
| the machine's stock menu | in game: a right click opens `container` with 27 slots at 300..326 |
| the interop (requester → crafter) | in game: `stored=8 crafted=8`; the same world without the requester leaves a machine at `stored=0 crafted=0` |
| the cross-block refusal | in game: `phase=not-enough-ingredients backoff=13`, restock → `stored=8 crafted=8`; and the no-room refusal reads `phase=network-full` on both blocks |
| the 8 upgrade slots | `blocked(no-parent-addon-surface)` — the RS `UpgradeContainer` |
| colouring recipes, tags, advancements, managers, monitor | `not ported` / `blocked(no-parent-addon-surface)` — `notes.md` §1 |

## Skills

`Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices`
(boot: `AGENTS.md`, `ARCHITECTURE.md` §3–§10, `ports/stepcrafter/{notes,VALIDATION,FINDINGS,CONTENT}.md`,
`horizon/docs/{modpacks,mods,native-mods}.md`, `ports/_shared/AGENT_MOD_STRUCTURE.md` §0,
`ports/_shared/build-slots-usage.md`, the SDK at `ports/_shared/sdk/0a875da/`).
