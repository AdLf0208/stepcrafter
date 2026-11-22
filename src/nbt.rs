//! Reading and writing a block entity's saved compound.
//!
//! Both machines keep their configuration in the block entity's saved NBT: the same handled types
//! (`Tag::String` for ids and names, `Tag::Int` for amounts and counters, `Tag::List` of strings for
//! the pattern's cells). These helpers are the whole of that vocabulary, so a key written by one
//! module is read by the other without a second interpretation.

use mod_api::{Compound, Tag};

/// Set one key, replacing whatever it held.
pub fn put(nbt: &mut Compound, name: &str, value: Tag) {
    nbt.insert(name.into(), value);
}

/// One key's value, if it is there.
pub fn tag<'a>(nbt: &'a Compound, name: &str) -> Option<&'a Tag> {
    nbt.get(&name.into())
}

/// Remove one key; a compound without it reads as unconfigured.
pub fn drop_key(nbt: &mut Compound, name: &str) {
    nbt.remove(&name.into());
}

/// One key as text, when it holds text.
#[must_use]
pub fn string(nbt: &Compound, name: &str) -> Option<String> {
    match tag(nbt, name) {
        Some(Tag::String(text)) => Some(text.to_string_lossy()),
        _ => None,
    }
}

/// One key as an amount, when it holds one.
#[must_use]
pub fn int(nbt: &Compound, name: &str) -> Option<i32> {
    match tag(nbt, name) {
        Some(Tag::Int(value)) => Some(*value),
        _ => None,
    }
}

/// One key as a list of texts, when it holds one.
#[must_use]
pub fn list(nbt: &Compound, name: &str) -> Option<Vec<String>> {
    let Some(Tag::List(items)) = tag(nbt, name) else {
        return None;
    };
    Some(
        items
            .iter()
            .filter_map(|item| match item {
                Tag::String(text) => Some(text.to_string_lossy()),
                _ => None,
            })
            .collect(),
    )
}

/// An amount as the `Int` the compound holds; a value past `i32`'s range saturates rather than
/// wrapping into a negative amount.
#[must_use]
pub fn int_of(value: u64) -> Tag {
    Tag::Int(i32::try_from(value).unwrap_or(i32::MAX))
}

/// A non-negative amount read back from an `Int`; a negative or absent one is zero.
#[must_use]
pub fn amount(nbt: &Compound, name: &str) -> u64 {
    int(nbt, name)
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(0)
}
