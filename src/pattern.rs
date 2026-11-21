//! Resolving a target item's own crafting recipe into the machine's step pattern.
//!
//! Upstream's pattern is a Refined Storage pattern item the player encodes. A Horizon mod cannot
//! read another mod's pattern item, so this port takes the pattern from the game's own recipe
//! table: the target item's `minecraft:crafting` recipe, whose pattern cells become the step's
//! ingredients and whose result becomes the step's output. That is the same object upstream
//! crafts — the recipe the pattern was encoded from — expressed with what the pin actually offers
//! (`RecipesHost::recipes_of_type`, `Recipe::ingredients`/`results`, `Ingredient::item_keys`).
//!
//! One host crossing per resolution: `recipes_of_type` and one `item_keys` per cell, at the moment
//! the player configures the machine. The tick never crosses to the recipe table.

use mod_api::{Host, Identifier, Ingredient};

use crate::runtime::Pattern;

/// The recipe type a step crafts through: vanilla's crafting table recipes.
pub const RECIPE_TYPE: &str = "minecraft:crafting";

/// A step pattern can hold at most a 3×3 grid; vanilla's shaped recipes never exceed it.
const MAX_CELLS: usize = 9;

/// Why a target could not be configured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolveError {
    /// The id is not an item this world knows.
    UnknownItem(String),
    /// The item has no `minecraft:crafting` recipe, so there is no step to take.
    NoRecipe(String),
    /// The recipe's shape does not fit a step (no ingredients, or more than nine cells).
    UnsupportedRecipe(String),
}

impl ResolveError {
    /// The line the command answers with.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnknownItem(id) => format!("stepcrafter: {id} is not an item this world knows"),
            Self::NoRecipe(id) => {
                format!("stepcrafter: {id} has no {RECIPE_TYPE} recipe, so it cannot be maintained")
            }
            Self::UnsupportedRecipe(id) => {
                format!("stepcrafter: {id} crafts from a recipe a step cannot take")
            }
        }
    }
}

/// The step pattern that crafts `target`, or why there is none.
///
/// Several recipes can make one item (a data pack adds one, or two shapes make the same thing):
/// the one with the fewest cells wins, then the earlier one in the recipe table, so re-configuring
/// the same target always yields the same pattern.
pub fn resolve(host: &dyn Host, target: &str) -> Result<Pattern, ResolveError> {
    let Ok(id) = Identifier::parse(target) else {
        return Err(ResolveError::UnknownItem(target.to_owned()));
    };
    let Some(item) = host.item_key(&id) else {
        return Err(ResolveError::UnknownItem(target.to_owned()));
    };
    let wanted = item.id(host);

    let mut best: Option<Pattern> = None;
    let mut saw_recipe = false;
    for recipe in host.recipes_of_type(RECIPE_TYPE).iter() {
        let [result] = recipe.results.as_slice() else {
            // A recipe with no result, or several, is not a craft one step can make.
            continue;
        };
        if result.item().id(host) != wanted {
            continue;
        }
        saw_recipe = true;
        let Some(cells) = cells(host, &recipe.ingredients) else {
            continue;
        };
        let Some(pattern) = Pattern::new(cells, wanted.clone(), result.count()) else {
            continue;
        };
        let better = best
            .as_ref()
            .is_none_or(|best| pattern.cells().len() < best.cells().len());
        if better {
            best = Some(pattern);
        }
    }
    match best {
        Some(pattern) => Ok(pattern),
        None if saw_recipe => Err(ResolveError::UnsupportedRecipe(target.to_owned())),
        None => Err(ResolveError::NoRecipe(target.to_owned())),
    }
}

/// The item ids of each ingredient's candidate set, or `None` when the recipe does not fit a step.
///
/// Every ingredient must resolve to items (`item_keys`: a list, an expanded tag or a standard
/// shape; a mod's own component predicate is `NotEnumerable` and is refused here rather than
/// risking a cell that can never be satisfied).
fn cells(host: &dyn Host, ingredients: &[Ingredient]) -> Option<Vec<Vec<String>>> {
    if ingredients.is_empty() || ingredients.len() > MAX_CELLS {
        return None;
    }
    let mut cells = Vec::with_capacity(ingredients.len());
    for ingredient in ingredients {
        let keys = ingredient.item_keys(host).ok()?;
        let mut ids: Vec<String> = keys.iter().map(|key| key.id(host)).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return None;
        }
        cells.push(ids);
    }
    Some(cells)
}
