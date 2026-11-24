//! The Step Requester's screen: its layout, its presses and what it draws.
//!
//! Upstream's requester GUI is 45 filter slots (9 × 5) plus 8 upgrade slots, each filter slot a
//! ghost slot with its own min/max editor. This port keeps the 45 cells and the same three values
//! per entry; the upgrade slots are the parent's `UpgradeContainer`, which no Horizon add-on can
//! reach (`notes.md` §1), so this screen edits the filters only.
//!
//! The cells are **drawn**, not menu slots: an entry is a resource, a band and a batch in the saved
//! container, and a menu slot can hold an item but not a band. A press on a cell or a button is a
//! menu button id ([`crate::runtime::Press`]), sent by the client (`screen_click`) and applied on the
//! server (`menu_button`); the numbers come back in the menu's data slots, the icons in the menu's
//! open payload.
//!
//! Everything that can be pure is: the layout constants, [`button_at`]'s hit test and the slot
//! order, so the geometry is unit-tested rather than only seen.

use mod_api::{ClientHost, DrawContext, Identifier, ItemKey, Screen, ScreenDrawEvent};

use crate::runtime::{self, MachinePress, Press, SCREEN_COLUMNS, SCREEN_ROWS};

/// The panel is vanilla's 176 wide, tall enough for five filter rows, three value rows and the
/// player's inventory.
pub const WIDTH: f32 = 176.0;
/// The panel's height: the inventory's first row at [`INVENTORY_Y`] plus vanilla's 82.
///
/// It must fit the game's 240-pixel GUI height (427 x 240 in the harness), because a taller panel is
/// drawn clipped and its lower rows are unreachable: the first layout here was 258 and the click
/// mapping sat 58 pixels above the drawn slots (measured in game). Vanilla's tallest container is
/// 222 for the same reason.
pub const HEIGHT: f32 = INVENTORY_Y + 82.0;
/// The player's inventory rows: below the grid and the three value rows.
pub const INVENTORY_Y: f32 = 156.0;
/// The inventory's left column (vanilla's 8).
pub const INVENTORY_X: f32 = 8.0;

/// The filter grid's top left and the size of one cell (a slot's 18 pixels).
const GRID_X: f32 = 8.0;
const GRID_Y: f32 = 18.0;
/// One cell: a vanilla slot's 18 pixels, the item drawn at 16 inside it.
pub const CELL: f32 = 18.0;

/// The two value rows under the grid: the first is `min` and `max` side by side, the second is
/// `batch` and the remove button. They end above the engine's "Inventory" label (`inventory_y - 16`),
/// which is drawn on the panel when the mod leaves the labels to the engine.
const VALUE_ROW_Y: [f32; 2] = [110.0, 128.0];
/// A value group's columns: its label, its `-` button, its number and its `+` button.
const MIN_GROUP: [f32; 4] = [8.0, 26.0, 40.0, 66.0];
const MAX_GROUP: [f32; 4] = [78.0, 96.0, 110.0, 136.0];
const BATCH_GROUP: [f32; 4] = [8.0, 38.0, 52.0, 78.0];
/// A value button: 12 pixels square, beside its group's number.
const BUTTON: f32 = 12.0;
/// The remove button, on the second value row beside the `batch` group.
const REMOVE: [f32; 4] = [100.0, 128.0, 60.0, 12.0];

/// The colours the panel draws with, vanilla's slot greys and the label grey.
const SLOT: u32 = 0xFF8B8B8B;
const SLOT_EDGE: u32 = 0xFF373737;
const SELECTED: u32 = 0x66FFFFFF;
const BUTTON_FACE: u32 = 0xFF6E6E6E;
const BUTTON_EDGE: u32 = 0xFF2B2B2B;
const TEXT: u32 = 0xFF404040;

/// The panel's layout: one hidden container slot (the menu needs one), no visible menu slot.
#[must_use]
pub fn layout() -> Screen {
    Screen {
        width: WIDTH,
        height: HEIGHT,
        // The container's one slot is hidden: the cells are drawn, and shift-clicks skip it.
        slots: vec![None],
        inventory_y: INVENTORY_Y,
        inventory_x: INVENTORY_X,
        own_labels: false,
        own_background: false,
        ..Screen::default()
    }
}

/// Which cell a point is in, if any: the row-major index of the filter grid.
#[must_use]
pub fn cell_at(at: [f32; 2]) -> Option<u8> {
    let [x, y] = at;
    if !(GRID_X..GRID_X + CELL * SCREEN_COLUMNS as f32).contains(&x)
        || !(GRID_Y..GRID_Y + CELL * SCREEN_ROWS as f32).contains(&y)
    {
        return None;
    }
    let column = ((x - GRID_X) / CELL).floor() as usize;
    let row = ((y - GRID_Y) / CELL).floor() as usize;
    let index = row * SCREEN_COLUMNS + column;
    u8::try_from(index).ok()
}

/// A rectangle's test, for the drawn buttons.
fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    let [x, y, w, h] = rect;
    (x..x + w).contains(&at[0]) && (y..y + h).contains(&at[1])
}

/// The menu button a press at `at` sends, or `None` when it is on nothing of the screen's.
#[must_use]
pub fn button_at(at: [f32; 2]) -> Option<i32> {
    if let Some(cell) = cell_at(at) {
        return Some(Press::Cell(cell).button());
    }
    let minus = |group: [f32; 4], row: usize| [group[1], VALUE_ROW_Y[row], BUTTON, BUTTON];
    let plus = |group: [f32; 4], row: usize| [group[3], VALUE_ROW_Y[row], BUTTON, BUTTON];
    let press = if inside(REMOVE, at) {
        Press::Remove
    } else if inside(minus(MIN_GROUP, 0), at) {
        Press::MinDown
    } else if inside(plus(MIN_GROUP, 0), at) {
        Press::MinUp
    } else if inside(minus(MAX_GROUP, 0), at) {
        Press::MaxDown
    } else if inside(plus(MAX_GROUP, 0), at) {
        Press::MaxUp
    } else if inside(minus(BATCH_GROUP, 1), at) {
        Press::BatchDown
    } else if inside(plus(BATCH_GROUP, 1), at) {
        Press::BatchUp
    } else {
        return None;
    };
    Some(press.button())
}

/// One cell's slot frame at the grid's column and row.
fn cell_rect(cell: usize) -> [f32; 4] {
    let column = (cell % SCREEN_COLUMNS) as f32;
    let row = (cell / SCREEN_COLUMNS) as f32;
    [GRID_X + column * CELL, GRID_Y + row * CELL, CELL, CELL]
}

/// A button's face: its edge, then its inside.
fn draw_button(draw: &mut dyn DrawContext, rect: [f32; 4], label: &str) {
    draw.fill(rect, BUTTON_EDGE);
    draw.fill(
        [rect[0] + 1.0, rect[1] + 1.0, rect[2] - 2.0, rect[3] - 2.0],
        BUTTON_FACE,
    );
    draw.text(label, [rect[0] + 3.0, rect[1] + 2.0], 0xFFFFFFFF, false);
}

/// Draw the screen: the cells (the selected one lit, each entry's resource), the selected entry's
/// three values and the buttons that step them.
///
/// The icons come from the menu's open payload (`menu_extra_data` → `screen_extra_data`) and the
/// numbers from the data slots, which change as the player presses; a resource set from this screen
/// shows its icon again when the screen is reopened (the payload is sent when the menu opens).
pub fn draw(host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let [ox, oy] = event.origin;
    let items = host
        .screen_extra_data()
        .map_or_else(Vec::new, |data| runtime::extra_items(&data));
    let selected = usize::try_from(event.data.get(1).copied().unwrap_or(0)).unwrap_or(0);
    let draw = &mut *event.draw;

    for cell in 0..SCREEN_COLUMNS * SCREEN_ROWS {
        let [x, y, w, h] = cell_rect(cell);
        let rect = [ox + x, oy + y, w, h];
        draw.fill(rect, SLOT);
        draw.fill([rect[0], rect[1], rect[2], 1.0], SLOT_EDGE);
        draw.fill([rect[0], rect[1], 1.0, rect[3]], SLOT_EDGE);
        draw.fill([rect[0] + rect[2] - 1.0, rect[1], 1.0, rect[3]], SLOT_EDGE);
        draw.fill([rect[0], rect[1] + rect[3] - 1.0, rect[2], 1.0], SLOT_EDGE);
        if cell == selected {
            draw.fill(rect, SELECTED);
        }
        if let Some(key) = items.get(cell).and_then(|item| item_key(host, item)) {
            draw.item(key, [rect[0] + 1.0, rect[1] + 1.0], 16.0);
        }
    }

    // The selected entry's three values, as the data slots carry them now.
    let value = |index: usize| event.data.get(index).copied().unwrap_or(0);
    let groups = [
        ("min", MIN_GROUP, 0usize, 2usize),
        ("max", MAX_GROUP, 0usize, 3usize),
        ("batch", BATCH_GROUP, 1usize, 4usize),
    ];
    for (label, group, row, index) in groups {
        let y = oy + VALUE_ROW_Y[row];
        draw.text(label, [ox + group[0], y + 2.0], TEXT, false);
        draw.text(
            &value(index).to_string(),
            [ox + group[2], y + 2.0],
            TEXT,
            false,
        );
        draw_button(draw, [ox + group[1], y, BUTTON, BUTTON], "-");
        draw_button(draw, [ox + group[3], y, BUTTON, BUTTON], "+");
    }
    let remove = [ox + REMOVE[0], oy + REMOVE[1], REMOVE[2], REMOVE[3]];
    draw_button(draw, remove, "remove");
}

/// An item id's key, `None` for one this world does not have.
fn item_key(host: &dyn ClientHost, item: &str) -> Option<ItemKey> {
    Identifier::parse(item)
        .ok()
        .and_then(|id| host.item_key(&id))
}

/// The machine's panel: vanilla's 176 wide, tall enough for its three stock rows, its
/// configuration row and the player's inventory.
pub const MACHINE_WIDTH: f32 = 176.0;
/// The player's inventory rows, under the pattern grid and the value row. It is the requester's
/// inventory row, so both panels are the same height and centre alike in the game's GUI.
pub const MACHINE_INVENTORY_Y: f32 = 156.0;
/// The machine's panel height: the inventory's first row plus vanilla's 82. Like the requester's
/// 238, it fits the game's 240-pixel GUI (a taller panel is clipped, §2g trap 3).
pub const MACHINE_HEIGHT: f32 = MACHINE_INVENTORY_Y + 82.0;

/// The stock grid's top left: the machine's own 27 slots, the same three chest rows, and they are
/// real menu slots (unlike the requester's drawn filter cells).
const STOCK_X: f32 = 8.0;
const STOCK_Y: f32 = 6.0;
const STOCK_COLUMNS: usize = 9;
/// One cell is 15 pixels: 27 stock rows (3) and 45 pattern rows (5) and the value row have to fit
/// above the engine's "Inventory" label at `MACHINE_INVENTORY_Y - 16`; 18-pixel cells do not.
const MACHINE_CELL: f32 = 15.0;
/// The pattern grid: upstream's 9 × 5 cells, one per pattern slot, in the same row-major order as
/// the requester's filter grid. Cell 0 keeps the first cell's place under the stock grid.
const MACHINE_GRID_X: f32 = 8.0;
const MACHINE_GRID_Y: f32 = 51.0;
const MACHINE_TARGET_COLUMNS: usize = 9;
const MACHINE_TARGET_ROWS: usize = 5;
/// All 45 of upstream's pattern slots are drawn, not just the first row.
pub const MACHINE_TARGETS: usize = MACHINE_TARGET_COLUMNS * MACHINE_TARGET_ROWS;

/// The `index`th pattern cell's bounds, left to right, row by row.
#[must_use]
fn target_cell(index: usize) -> Option<[f32; 4]> {
    (index < MACHINE_TARGETS).then(|| {
        let column = (index % MACHINE_TARGET_COLUMNS) as f32;
        let row = (index / MACHINE_TARGET_COLUMNS) as f32;
        [
            MACHINE_GRID_X + column * MACHINE_CELL,
            MACHINE_GRID_Y + row * MACHINE_CELL,
            MACHINE_CELL,
            MACHINE_CELL,
        ]
    })
}

/// The band's row under the pattern grid: `min`'s group then `max`'s, each a label, `-`, its number
/// and `+`.
const MACHINE_VALUE_Y: f32 = 127.0;
const MACHINE_MIN_GROUP: [f32; 4] = [8.0, 30.0, 44.0, 66.0];
const MACHINE_MAX_GROUP: [f32; 4] = [88.0, 110.0, 124.0, 146.0];

/// The machine's rows fit inside its panel, checked when the crate compiles: the stock grid above
/// the target cell, the cell above the band's row, that row above the engine's "Inventory" label,
/// and the inventory inside the height the panel declares.
const _: () = {
    assert!(
        STOCK_Y + MACHINE_CELL * (crate::machine::SLOTS / STOCK_COLUMNS) as f32 <= MACHINE_GRID_Y
    );
    assert!(MACHINE_GRID_Y + MACHINE_CELL * MACHINE_TARGET_ROWS as f32 <= MACHINE_VALUE_Y);
    assert!(MACHINE_GRID_X + MACHINE_TARGET_COLUMNS as f32 * MACHINE_CELL <= MACHINE_WIDTH);
    assert!(MACHINE_VALUE_Y + BUTTON <= MACHINE_INVENTORY_Y - 16.0);
    assert!(MACHINE_INVENTORY_Y + 82.0 <= MACHINE_HEIGHT + 0.01);
    assert!(MACHINE_HEIGHT <= 240.0);
};

/// The machine's panel: its 27 stock slots as menu slots, the target cell and the band's buttons,
/// then the player's inventory.
#[must_use]
pub fn machine_layout() -> Screen {
    let slots = (0..crate::machine::SLOTS)
        .map(|slot| {
            let column = (slot % STOCK_COLUMNS) as f32;
            let row = (slot / STOCK_COLUMNS) as f32;
            Some([
                STOCK_X + column * MACHINE_CELL,
                STOCK_Y + row * MACHINE_CELL,
            ])
        })
        .collect();
    Screen {
        width: MACHINE_WIDTH,
        height: MACHINE_HEIGHT,
        slots,
        inventory_y: MACHINE_INVENTORY_Y,
        inventory_x: INVENTORY_X,
        own_labels: false,
        own_background: false,
        ..Screen::default()
    }
}

/// The menu button a press on the machine's panel sends, or `None` when it is on a stock slot or
/// nothing of the screen's.
#[must_use]
pub fn machine_button_at(at: [f32; 2]) -> Option<i32> {
    for index in 0..MACHINE_TARGETS {
        if let Some(cell) = target_cell(index)
            && inside(cell, at)
        {
            return Some(MachinePress::Cell(index as u8).button());
        }
    }
    let minus = |group: [f32; 4]| [group[1], MACHINE_VALUE_Y, BUTTON, BUTTON];
    let plus = |group: [f32; 4]| [group[3], MACHINE_VALUE_Y, BUTTON, BUTTON];
    let press = if inside(minus(MACHINE_MIN_GROUP), at) {
        MachinePress::MinDown
    } else if inside(plus(MACHINE_MIN_GROUP), at) {
        MachinePress::MinUp
    } else if inside(minus(MACHINE_MAX_GROUP), at) {
        MachinePress::MaxDown
    } else if inside(plus(MACHINE_MAX_GROUP), at) {
        MachinePress::MaxUp
    } else {
        return None;
    };
    Some(press.button())
}

/// Draw the machine's panel over the engine's: the 45 pattern cells (their items from the open
/// payload) and the focused cell's band, as the data slots carry it.
///
/// The engine draws the plain panel, the stock slots' frames and the player's inventory; the drawn
/// cells and buttons are this function's, over the panel and under the items and labels, so every
/// coordinate must add `event.origin`.
pub fn machine_draw(host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
    let [ox, oy] = event.origin;
    let (selected, targets) = host
        .screen_extra_data()
        .map(|data| runtime::machine_extra_items(&data))
        .unwrap_or((0, Vec::new()));
    let draw = &mut *event.draw;

    for index in 0..MACHINE_TARGETS {
        let Some([x, y, w, h]) = target_cell(index) else {
            break;
        };
        let rect = [ox + x, oy + y, w, h];
        draw.fill(rect, SLOT);
        // The focused cell gets a brighter frame so a player can see which band the buttons step.
        let edge = if index == selected { TEXT } else { SLOT_EDGE };
        draw.fill([rect[0], rect[1], rect[2], 1.0], edge);
        draw.fill([rect[0], rect[1], 1.0, rect[3]], edge);
        draw.fill([rect[0] + rect[2] - 1.0, rect[1], 1.0, rect[3]], edge);
        draw.fill([rect[0], rect[1] + rect[3] - 1.0, rect[2], 1.0], edge);
        if let Some(item) = targets.get(index).filter(|item| !item.is_empty())
            && let Some(key) = item_key(host, item)
        {
            // A 15-pixel cell cannot show a 16-pixel icon: draw it at 13 inside the frame.
            draw.item(key, [rect[0] + 1.0, rect[1] + 1.0], 13.0);
        }
    }

    let value = |index: usize| event.data.get(index).copied().unwrap_or(0);
    for (label, group, index) in [
        ("min", MACHINE_MIN_GROUP, 0usize),
        ("max", MACHINE_MAX_GROUP, 1usize),
    ] {
        let row = oy + MACHINE_VALUE_Y;
        draw.text(label, [ox + group[0], row + 2.0], TEXT, false);
        draw.text(
            &value(index).to_string(),
            [ox + group[2], row + 2.0],
            TEXT,
            false,
        );
        draw_button(draw, [ox + group[1], row, BUTTON, BUTTON], "-");
        draw_button(draw, [ox + group[3], row, BUTTON, BUTTON], "+");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The panel's rows fit inside it, checked when the crate compiles: the grid above the value
    /// rows, those above the inventory, and the inventory inside the height the screen declares.
    const _: () = {
        // The grid ends above the value rows; those end above the engine's "Inventory" label
        // (`inventory_y - 16`), which the panel draws when the mod leaves its labels to it; and the
        // panel fits the game's 240-pixel GUI, or its lower rows are unreachable.
        assert!(GRID_Y + CELL * SCREEN_ROWS as f32 <= VALUE_ROW_Y[0]);
        assert!(VALUE_ROW_Y[1] + BUTTON <= INVENTORY_Y - 16.0);
        assert!(INVENTORY_Y + 82.0 <= HEIGHT + 0.01);
        assert!(HEIGHT <= 240.0);
    };

    /// The grid is upstream's 9 × 5: the first cell's top-left is its only point that is inside it,
    /// and the 45th cell is the last that answers.
    #[test]
    fn the_grid_is_nine_by_five_and_the_cells_do_not_overlap() {
        assert_eq!(
            cell_at([GRID_X + 1.0, GRID_Y + 1.0]),
            Some(0),
            "the first cell"
        );
        assert_eq!(
            cell_at([GRID_X + CELL * 8.0 + 17.0, GRID_Y + 17.0]),
            Some(8)
        );
        assert_eq!(cell_at([GRID_X + 1.0, GRID_Y + CELL * 4.0 + 1.0]), Some(36));
        assert_eq!(
            cell_at([GRID_X + CELL * 8.0 + 17.0, GRID_Y + CELL * 4.0 + 17.0]),
            Some(44)
        );
        assert_eq!(cell_at([GRID_X - 1.0, GRID_Y]), None, "left of the grid");
        assert_eq!(
            cell_at([GRID_X, GRID_Y + CELL * 5.0]),
            None,
            "below the grid"
        );
        assert_eq!(
            cell_at([GRID_X + CELL * 9.0, GRID_Y]),
            None,
            "right of the grid"
        );
    }

    /// Each drawn button is one menu button id, and the point outside every button is nothing.
    #[test]
    fn the_buttons_are_the_presses_the_server_applies() {
        let minus = |group: [f32; 4], row: usize| [group[1], VALUE_ROW_Y[row], BUTTON, BUTTON];
        let plus = |group: [f32; 4], row: usize| [group[3], VALUE_ROW_Y[row], BUTTON, BUTTON];
        for (rect, press) in [
            (minus(MIN_GROUP, 0), Press::MinDown),
            (plus(MIN_GROUP, 0), Press::MinUp),
            (minus(MAX_GROUP, 0), Press::MaxDown),
            (plus(MAX_GROUP, 0), Press::MaxUp),
            (minus(BATCH_GROUP, 1), Press::BatchDown),
            (plus(BATCH_GROUP, 1), Press::BatchUp),
            (REMOVE, Press::Remove),
        ] {
            let centre = [rect[0] + 2.0, rect[1] + 2.0];
            assert_eq!(
                button_at(centre),
                Some(press.button()),
                "the rect {rect:?} is {press:?}"
            );
            assert_eq!(Press::of(press.button()), Some(press));
        }
        // Between the grid and the value rows nothing is drawn, so nothing is pressed.
        assert_eq!(button_at([GRID_X, VALUE_ROW_Y[0] - 2.0]), None);
        assert_eq!(button_at([WIDTH - 2.0, HEIGHT - 2.0]), None);
        // A cell press outranks nothing, but the value rows and the grid do not overlap.
        assert_eq!(button_at([GRID_X + 1.0, GRID_Y + 1.0]), Some(100));
    }

    /// The panel is tall enough for its own rows: the grid, the three value rows, then the
    /// inventory below them all, inside the height the screen declares.
    #[test]
    fn the_panel_holds_its_rows_and_the_inventory() {
        // The rows' fit is a compile-time fact (see this module's `const` block); what is left to
        // check here is the declaration the engine reads.
        let screen = layout();
        assert_eq!((screen.width, screen.height), (176.0, 238.0));
        assert_eq!(screen.inventory_y, 156.0);
        assert_eq!(
            screen.slots,
            vec![None],
            "the container's one slot is hidden: the cells are drawn"
        );
    }

    /// The machine's 27 stock slots are menu slots, placed in the chest's three rows, and its
    /// panel is the requester's 176 × 238 declaration, so both panels centre alike in game.
    #[test]
    fn the_machines_stock_slots_are_its_containers_twenty_seven() {
        let screen = machine_layout();
        assert_eq!((screen.width, screen.height), (176.0, 238.0));
        assert_eq!(screen.inventory_y, 156.0);
        assert_eq!(screen.slots.len(), 27, "every stock slot is a menu slot");
        assert_eq!(screen.slots[0], Some([STOCK_X, STOCK_Y]));
        assert_eq!(
            screen.slots[26],
            Some([STOCK_X + 8.0 * MACHINE_CELL, STOCK_Y + 2.0 * MACHINE_CELL])
        );
        assert!(
            screen.slots.iter().all(Option::is_some),
            "no stock slot is hidden"
        );
    }

    /// The machine's cell and buttons are the presses the server applies, and a press on a stock
    /// slot is the menu's (it must not be swallowed as a button).
    #[test]
    fn the_machines_cell_and_buttons_are_the_presses_the_server_applies() {
        let minus = |group: [f32; 4]| [group[1], MACHINE_VALUE_Y, BUTTON, BUTTON];
        let plus = |group: [f32; 4]| [group[3], MACHINE_VALUE_Y, BUTTON, BUTTON];
        for (rect, press) in [
            (
                target_cell(0).expect("the first cell"),
                MachinePress::Cell(0),
            ),
            (
                target_cell(1).expect("the second cell"),
                MachinePress::Cell(1),
            ),
            (minus(MACHINE_MIN_GROUP), MachinePress::MinDown),
            (plus(MACHINE_MIN_GROUP), MachinePress::MinUp),
            (minus(MACHINE_MAX_GROUP), MachinePress::MaxDown),
            (plus(MACHINE_MAX_GROUP), MachinePress::MaxUp),
        ] {
            let centre = [rect[0] + 2.0, rect[1] + 2.0];
            assert_eq!(
                machine_button_at(centre),
                Some(press.button()),
                "the rect {rect:?} is {press:?}"
            );
            assert_eq!(MachinePress::of(press.button()), Some(press));
        }
        assert_eq!(
            machine_button_at([STOCK_X + 1.0, STOCK_Y + 1.0]),
            None,
            "a press on a stock slot belongs to the menu, not the screen"
        );
        assert_eq!(
            machine_button_at([MACHINE_WIDTH - 2.0, MACHINE_HEIGHT - 2.0]),
            None
        );
    }

    /// Every one of the 45 cells has its own pixel and its own button id: the grid is upstream's
    /// 9 × 5 in row-major order, and none of them lands on a stock slot or a value button.
    #[test]
    fn every_one_of_the_forty_five_cells_presses_its_own_slot() {
        assert_eq!(MACHINE_TARGETS, 45);
        for index in 0..MACHINE_TARGETS {
            let [x, y, w, h] = target_cell(index).expect("one of the 45 cells");
            let centre = [x + w / 2.0, y + h / 2.0];
            let press = MachinePress::Cell(index as u8);
            assert_eq!(
                machine_button_at(centre),
                Some(press.button()),
                "cell {index}'s centre must press its own slot"
            );
            assert_eq!(MachinePress::of(press.button()), Some(press));
        }
        // Cell 0 is the grid's top left; cell 44 is its bottom right.
        assert_eq!(target_cell(0), Some([8.0, 51.0, 15.0, 15.0]));
        assert_eq!(target_cell(44), Some([128.0, 111.0, 15.0, 15.0]));
    }

    /// The panel must draw upstream's whole 9 × 5 pattern grid, not just its first row: the 45th
    /// cell's pixel is the one a player clicks to configure slot 44 from the panel.
    #[test]
    fn the_forty_fifth_cell_is_on_the_machine_panel() {
        // The computed centre of cell 44: column 8, row 4 of a 15-pixel grid at (8, 51).
        let centre = [135.0, 118.0];
        assert_eq!(
            machine_button_at(centre),
            Some(MachinePress::Cell(44).button()),
            "cell 44's centre must press the 45th pattern slot"
        );
    }

    /// The machine's panel is the requester's height so the five pattern rows, the value row and
    /// the inventory all fit inside the game's 240-pixel GUI (a taller panel is clipped).
    #[test]
    fn the_machine_panel_is_the_requesters_height() {
        let layout = machine_layout();
        assert_eq!((layout.width, layout.height), (176.0, 238.0));
        assert_eq!(layout.inventory_y, 156.0);
    }
}
