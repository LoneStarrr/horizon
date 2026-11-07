//! Density function types a mod registers, handed over once at startup and
//! looked up when the noise router's JSON names a type the engine doesn't
//! know (`"type": "mymod:wave"`). The decoded function joins the density graph
//! as a custom node, so the noise pipeline treats it like any other. Runs on
//! the worldgen worker threads, so a type keeps no mutable state.
use horizon_core::density::graph::CustomDensity;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// A density function type's engine side.
pub trait ModDensityType: Send + Sync {
    /// Decode one use of the type from its JSON object (the `type` field
    /// included), or say why it is invalid. The same JSON gives the same
    /// function.
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn CustomDensity>, String>;
}

/// The installed types, `modid:wave` → type. Set once, before any world opens.
pub type ModDensityTypes = BTreeMap<String, Arc<dyn ModDensityType>>;

static MOD_DENSITY_TYPES: OnceLock<ModDensityTypes> = OnceLock::new();
static EMPTY: ModDensityTypes = BTreeMap::new();

/// Install the process-wide table once; a second installation keeps the first.
pub fn install_mod_density_types(types: ModDensityTypes) -> Result<(), ModDensityTypes> {
    MOD_DENSITY_TYPES.set(types)
}

/// The installed types: empty until the executable installs the mods'.
pub fn mod_density_types() -> &'static ModDensityTypes {
    MOD_DENSITY_TYPES.get().unwrap_or(&EMPTY)
}
