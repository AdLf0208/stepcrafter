# Step Crafter — validation (0.2.7)

Everything here is a command that was run on 2026-11-23, with its artefact under
`ports/_shared/evidence/stepcrafter/` (unit/build side) and
`ports/_shared/evidence/stepcrafter-e2e/` (game side). The pin is
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.
This slice is **the queued ask** (`notes.md` §2h): `Request` was one `Int` in the crafter's
compound, so a second requester asking the same crafter was never queued and both read the same
single inventory as their own. The ask is now a list of `(requester, item, remaining)` served head
first and credited to the asker whose step it was, with each requester's readout naming its own
share. The package is **84,810 B** sha256
`968321fd7ef5c64b9825fbe4eb87a1b0c4910512cab89f1d5ebe27f225c24bbc`, version `0.2.7`.

## The ladder

| step | command | result | artefact |
|---|---|---|---|
| red (unit) | `cargo test --offline` with `Queue`/`Queued`/`FailedAsk`/`settle` as stubs | `FAILED. 64 passed; 8 failed` (exit 101): `the first ask is queued`, `the failed task times out` (`left: (Crafting, 0) / right: (NotEnoughIngredients, 20)`), `assertion failed: queue.ask(ASKER_A, "minecraft:torch", 4)` | `stepcrafter/red-unit-queue.log` |
| red (game) | the **0.2.6** export (`~/.cache/horizon-e2e/0a875da/stepcrafter/`, version 0.2.6) run with this leg: `SOCK=… bash ports/stepcrafter/tools/e2e.sh` | `FAIL: the second requester is not queued behind the first with its own share of zero: … stored=4 phase=not-enough-ingredients … requests=3` — the first's torches read as the second's, no queue | `stepcrafter-e2e/e2e-red-queue-0.2.6.log` |
| unit tests | `cargo test --offline` | **73 passed, 0 failed**, `Compiling stepcrafter v0.2.7` | `stepcrafter/sdk-test.log` |
| format | `rustfmt --check` on the port's eight sources (not `cargo fmt --all`: it would reach into `.horizon/sdk`) | `FMT_EXIT:0` | `stepcrafter/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `Checking stepcrafter v0.2.7`, `CLIPPY_EXIT:0`, 0 warnings | `stepcrafter/sdk-clippy.log` |
| two builds | `rm -f Cargo.lock` + `cargo horizon-mod build`, twice, then `cmp` | `cmp: identical`, sha256 `968321fd…` twice, **0** `Cargo.lock` entries, 39 files | `stepcrafter/sdk-build-twice.log` |
| package check | `cargo horizon-mod check stepcrafter.hmod` | `stepcrafter 0.2.7 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, `84810 bytes, SHA-256 968321fd…`, `CHECK_EXIT:0` | `stepcrafter/sdk-check.log` |
| fits, same build | `cargo horizon-mod fits stepcrafter.hmod --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0a875da.log` |
| fits, other build | `cargo horizon-mod fits stepcrafter.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `stepcrafter/sdk-fits-0b14f8a.log` |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` | **1/1** (`package matches the tree`, `no packed Cargo.lock`, `tests`/`clippy`/`check`/`fits`/`skills gate` all ok) | `stepcrafter/verify-sdk-package.log` |
| e2e logs | `python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict` | `logs=12 ok=12 missing-exit=0 bad-exit=0` | §The in-game leg |

Every package log names the sha it ran against (`968321fd…`), the delivered package's.

## The in-game leg

The game is built by the launcher, not by hand:
`ports/tools/build-slot.sh run --job W1-stepcrafter --port ports/stepcrafter --step build --
ports/tools/launcher-game.sh build stepcrafter` → `native-build … Compiling stepcrafter (1 of 1)` →
`native-0a875da-35dc74f3a6db built and selected` → exported to
`~/.cache/horizon-e2e/0a875da/stepcrafter/` (`stepcrafter-e2e/game-build.log`). The version bump made
it recompile, so the exported binary is the delivered sources, not a reused manifest.

Each run is one command, through the slot registry, with the harness's own scratch root
(`E2E_TMP=/home/adlf/.cache/horizon-e2e/scratch`, never `/tmp`). Two runs of the same port are
**not** concurrent: the e2e guard keys its ctl shim by mod, so a parallel run deletes the other's.

| run | command | verdict | log |
|---|---|---|---|
| main | `SOCK=… bash tools/e2e.sh` (`GAME` = the built game, `CONTROL_GAME` = the same Nightly without it, in-run control leg) | `PASS` + `EXIT:0` | `stepcrafter-e2e/e2e-launcher.log` |
| control | `GAME=$CONTROL_GAME CONTROL_GAME= …` | `FAIL` + `EXIT:1` (`expected stepcrafter:step_crafter at 200 64 0, got minecraft:air`) | `stepcrafter-e2e/e2e-control.log` |
| selftest (requester) | `SELFTEST=1 …` | `FAIL` + `EXIT:1` (`the requester reached stored=8 …`) | `stepcrafter-e2e/e2e-selftest.log` |
| selftest (no room) | `SELFTEST=capacity …` | `FAIL` + `EXIT:1` (`the step fit no slot and served nothing …`) | `stepcrafter-e2e/e2e-selftest-capacity.log` |
| selftest (components) | `SELFTEST=components-room …` | `FAIL` + `EXIT:1` (`the named stack counted no room and the step was refused …`) | `stepcrafter-e2e/e2e-selftest-components-room.log` |
| selftest (take) | `SELFTEST=take …` | `FAIL` + `EXIT:1` (`the taken stacks kept their components …`) | `stepcrafter-e2e/e2e-selftest-take.log` |
| selftest (remainders) | `SELFTEST=remainder …` | `FAIL` + `EXIT:1` (`the step left the buckets its milk leaves …`) | `stepcrafter-e2e/e2e-selftest-remainder.log` |
| selftest (filters) | `SELFTEST=filters …` | `FAIL` + `EXIT:1` (`the requester keeps both filter entries (filters=2) …`) | `stepcrafter-e2e/e2e-selftest-filters.log` |
| selftest (screen) | `SELFTEST=screen …` | `FAIL` + `EXIT:1` (`the requester opens its filter screen (container) where today's defect opens none (playing) …`) | `stepcrafter-e2e/e2e-selftest-screen.log` |
| **selftest (queue)** | `SELFTEST=queue …` | `FAIL` + `EXIT:1` (`the second requester was served its own four torches where today's single ask loses it …`) | `stepcrafter-e2e/e2e-selftest-queue.log` |
| red (previous package) | the 0.2.6 export run with this leg, no rebuild | `FAIL` + `EXIT:1` (`the second requester is not queued behind the first with its own share of zero: … stored=4 …`) | `stepcrafter-e2e/e2e-red-queue-0.2.6.log` |
| red (older package) | the 0.2.3 export run with the 0.2.4 leg, kept from that slice | `FAIL` + `EXIT:1` (`result=crafted` beside the unchanged named stack) | `stepcrafter-e2e/e2e-red-components-0.2.3.log` |

```
$ python3 ports/tools/check-e2e-logs.py stepcrafter-e2e --strict
ok       stepcrafter-e2e        e2e-control.log            FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-launcher.log           PASS + EXIT:0
ok       stepcrafter-e2e        e2e-red-components-0.2.3.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-red-queue-0.2.6.log    FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-capacity.log  FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-components-room.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-filters.log   FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-queue.log     FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-remainder.log FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-screen.log    FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest-take.log      FAIL + EXIT:1
ok       stepcrafter-e2e        e2e-selftest.log           FAIL + EXIT:1
logs=12  ok=12  missing-exit=0  bad-exit=0
```

## The slice: queued asks, in game

One run of the main leg (phase 24 of `tools/e2e.sh`); every value is parsed from a `ctl` reply,
never from the absence of an error:

```
  one step's ingredients: stepcrafter 221 64 0 target=minecraft:torch min=0 max=0 stored=0
    phase=idle cells=2 result=minecraft:torch:4 crafted=0 queued=0 asks=none served=none
    items=2 stock=minecraft:coal:1,minecraft:stick:1
  after 40 ticks, the first:  … kind=requester … min=8 max=8 batch=4 stored=4 served=4 queued=1
    phase=not-enough-ingredients crafter=221,64,0 backoff=19 requests=2
    entries=minecraft:torch|8|8|4|4|4|1|not-enough-ingredients|221,64,0|19
  after 40 ticks, the second: … kind=requester … min=8 max=8 batch=4 stored=0 served=0 queued=1
    phase=running crafter=221,64,0 backoff=0 requests=1
    entries=minecraft:torch|8|8|4|0|0|1|running|221,64,0|0
  after 40 ticks, the crafter: … stored=4 phase=not-enough-ingredients … crafted=4 queued=2
    asks=222,64,0|minecraft:torch|4;222,64,2|minecraft:torch|4
    served=222,64,0|minecraft:torch|4 items=4 stock=minecraft:torch:4
  after the restock, the first:  … stored=8 served=8 queued=0 phase=idle …
  after the restock, the second: … stored=4 served=4 queued=1 phase=not-enough-ingredients …
  after the restock, the crafter: … stored=12 … crafted=12 queued=1
    asks=222,64,2|minecraft:torch|4 served=222,64,0|minecraft:torch|8;222,64,2|minecraft:torch|4
    items=12 stock=minecraft:torch:12
```

The brief's assertion, read back: after 40 ticks the first requester's own share is `stored=4
served=4`, the second is `stored=0 queued=1 phase=running`, and the crafter holds **both** asks in
order (`asks=…`), having credited its one step to the first (`served=…|4`). A restock of two steps
then serves the first's kept ask (it reaches its maximum of 8) and the second's (`served=4`), three
steps in all (`crafted=12`).

**Differentials / controls:** the previous package (0.2.6) run with this same leg reads the second
requester at `stored=4` — the first's torches as its own — and the leg fails on it
(`e2e-red-queue-0.2.6.log`); the same world without the package has no block at all
(`e2e-control.log`); and the whole scenario's unchanged legs (band, room, components, remainders,
take, screen, filters) still pass in the same run.

**Fail-capability:** `SELFTEST=queue` asserts the fixed behaviour (the second requester reaches
`served=4`, which today's single ask cannot produce) and ends the run non-zero; the seven older
selftests still invert their own assertions and still end non-zero.

## Coverage (family → evidence)

`notes.md` §10 is the full table (updated for this slice). The families this slice changed:

| family | evidence |
|---|---|
| two requesters on one crafter: the second's ask is queued, not lost | `a_second_requester_is_queued_beside_the_first`, `the_queue_serves_in_a_fixed_order_whatever_order_it_was_written`; in game `queued=2 asks=222,64,0\|…;222,64,2\|…` and the second's `stored=0 queued=1 phase=running` |
| a step's product is credited to the ask it served | `a_step_is_credited_to_the_ask_it_served`, `a_requester_holds_one_ask_per_resource`; in game `served=222,64,0\|minecraft:torch\|4` after one step, the second's share 0 |
| a requester's share survives a restart and the queue round-trips | `the_queue_round_trips_through_its_saved_form`, `the_queue_drops_asks_that_can_never_be_served`, the machine's `the_shared_read_is_the_queue_and_the_phase`, `a_legacy_single_ask_is_not_a_queued_ask` |
| the refusal names one ask and keeps it queued | `a_failed_ask_keeps_its_place_and_names_its_requester`, `a_refusal_settles_the_entry_and_a_served_ask_resumes_it`; in game the requester at `215` reads `phase=not-enough-ingredients queued=1` and the restock serves the kept ask (`stored=8 crafted=8`) |
| a shared crafter serves in a fixed, fair order | in game the first (lower position) is served first, and after the restock the second is served its own four |

## Skills

`Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices`
(boot: `AGENTS.md`, `ARCHITECTURE.md` §3–§10, `ports/stepcrafter/{notes,VALIDATION,FINDINGS,CONTENT}.md`,
`horizon/docs/{modpacks,mods,native-mods}.md`, `ports/_shared/AGENT_MOD_STRUCTURE.md` §0,
`ports/_shared/build-slots-usage.md`, the SDK at `ports/_shared/sdk/0a875da/`).
