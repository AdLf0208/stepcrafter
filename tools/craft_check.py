#!/usr/bin/env python3
"""Craft the one recipe the Step Crafter package carries, in a real crafting table.

Called by `tools/e2e.sh` with the open game's control socket. Uses the harness's crafting grid
(`ctl screen`: slots 100-108 row-major, result 200), gives the ingredients and asserts:

1. `data/stepcrafter/recipe/step_crafter.json` (upstream's shape `ECE/AMA/EDE`, re-rooted on
   vanilla ingredients — see notes.md) crafts exactly one `stepcrafter:step_crafter`, and the five
   ingredients are consumed;
2. the same ingredients in a pattern the recipe does not declare craft nothing — the control that
   makes case 1 an assertion rather than a coincidence.

The table is replaced before each case, so an untaken result cannot leak into the next grid.
Exits 1 with the observed result on the first mismatch.
"""
import json
import subprocess
import sys

# Row-major slots of the table screen.
GRID = ["100", "101", "102", "103", "104", "105", "106", "107", "108"]
# The recipe's own shape: E at the four corners, C top middle, A at the middle sides, M in the
# middle, D bottom middle.
IRON = ["100", "102", "106", "108"]
REDSTONE = ["101"]
GOLD = ["103", "105"]
CRAFTER = ["104"]
HOPPER = ["107"]
# The same items, moved one slot left where they fit: not the declared pattern.
WRONG_IRON = ["100", "102", "103", "106"]


def main():
    sock, ctl, x, y, z = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]

    def run(*args):
        out = subprocess.run(
            [ctl, "--socket", sock, "--timeout", "30", *args],
            text=True,
            capture_output=True,
        )
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            raise SystemExit(f"ctl {' '.join(args)}: {out.stdout}{out.stderr}")

    def wait(ticks=4):
        run("wait", str(ticks))

    def slots():
        screen = run("screen")
        if "slots" not in screen:
            raise SystemExit(f"no menu is open: {screen}")
        return screen["slots"]

    def find(item):
        for slot in slots():
            if slot.get("item") == item:
                return slot["slot"]
        raise SystemExit(f"missing ingredient {item} in the inventory")

    def result():
        """The result slot's stack, or None: an empty slot has `item: null`."""
        stack = [s for s in slots() if s["slot"] == 200]
        if not stack or not stack[0].get("item"):
            return None
        return stack[0]

    def place(item, targets):
        slot = find(item)
        run("click-slot", str(slot))
        wait(1)
        for target in targets:
            run("click-slot", str(target), "right")
            wait(1)
        run("click-slot", str(slot))
        wait(2)

    def setup(items):
        try:
            run("close")
        except SystemExit:
            pass
        wait(5)
        # Replace the table so its block entity, and so its grid, is fresh for this case.
        run("setblock", x, y, z, "minecraft:air")
        run("setblock", x, y, z, "minecraft:crafting_table")
        wait(2)
        run("command", f"tp @s {float(x) - 0.5} {y} {float(z) + 0.5}")
        wait(3)
        run("command", "clear @s")
        for item, count in items:
            run("give", item, str(count))
        wait(3)
        run("interact", x, y, z)
        wait(5)
        screen = {}
        for _ in range(10):
            screen = run("screen")
            if "slots" in screen:
                return
            wait(2)
        raise SystemExit(f"the crafting table did not open a menu: {screen}")

    def counts():
        return run("inventory").get("counts", {})

    failures = []

    def expect(label, items, placements, want_item, want_count, consume=None):
        setup(items)
        for item, targets in placements:
            place(item, targets)
        got = result()
        print(f"{label} ->", json.dumps(got))
        if not got or got.get("item") != want_item or got.get("count") != want_count:
            failures.append(f"{label}: expected {want_count} {want_item}, got {got}")
            return
        run("click-slot", "200", "shift")
        wait(3)
        run("close")
        wait(3)
        got_counts = counts()
        print("  inventory after taking it ->", json.dumps(got_counts))
        if got_counts.get(want_item, 0) != want_count:
            failures.append(
                f"{label}: the inventory holds {got_counts.get(want_item, 0)} {want_item}, "
                f"expected {want_count}"
            )
        for item in consume or []:
            if got_counts.get(item, 0) != 0:
                failures.append(
                    f"{label}: {got_counts.get(item, 0)} {item} left, so the grid was not consumed"
                )

    def expect_empty(label, items, placements):
        setup(items)
        for item, targets in placements:
            place(item, targets)
        got = result()
        print(f"{label} ->", json.dumps(got))
        if got is not None:
            failures.append(f"{label}: control crafted {got.get('count')} {got.get('item')}")

    ingredients = [
        ("minecraft:iron_ingot", 4),
        ("minecraft:redstone", 1),
        ("minecraft:gold_ingot", 2),
        ("minecraft:crafter", 1),
        ("minecraft:hopper", 1),
    ]
    expect(
        "the carried step_crafter recipe (ECE/AMA/EDE)",
        ingredients,
        [
            ("minecraft:iron_ingot", IRON),
            ("minecraft:redstone", REDSTONE),
            ("minecraft:gold_ingot", GOLD),
            ("minecraft:crafter", CRAFTER),
            ("minecraft:hopper", HOPPER),
        ],
        "stepcrafter:step_crafter",
        1,
        consume=[
            "minecraft:iron_ingot",
            "minecraft:redstone",
            "minecraft:gold_ingot",
            "minecraft:crafter",
            "minecraft:hopper",
        ],
    )
    expect_empty(
        "the same ingredients in a pattern the recipe does not declare",
        ingredients,
        [
            ("minecraft:iron_ingot", WRONG_IRON),
            ("minecraft:redstone", REDSTONE),
            ("minecraft:gold_ingot", GOLD),
            ("minecraft:crafter", CRAFTER),
            ("minecraft:hopper", HOPPER),
        ],
    )

    run("close")
    wait(3)
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        raise SystemExit(1)
    print(
        "recipe PASS: ECE/AMA/EDE crafts exactly one stepcrafter:step_crafter from 4 iron, "
        "1 redstone, 2 gold, 1 crafter and 1 hopper and consumes them, and the same items in "
        "another pattern craft nothing"
    )


if __name__ == "__main__":
    main()
