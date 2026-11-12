use super::*;
/// The tree parts registered at startup, for the executable to hand to the engine.
pub(crate) fn installed_tree_part_table() -> Option<&'static crate::world::tree_parts::TreePartTable>
{
    installed().map(|installed| &installed.tree_parts)
}

impl dyn Registrar + '_ {
    /// Register trunk placer type `name` (`spiral_trunk_placer` or `mymod:spiral_trunk_placer`)
    /// once at startup: a `minecraft:tree` config in a data pack with
    /// `"trunk_placer": {"type": "mymod:spiral_trunk_placer", ...}` then places its trunk by
    /// `ty`. The id is in the calling mod's namespace; another mod's namespace, the `minecraft`
    /// namespace, an empty name, a name outside vanilla's resource location rules (lowercase
    /// letters, digits and `_ . - /`) and an id registered twice as a trunk placer refuse the
    /// mods at startup, naming the mod.
    pub fn register_trunk_placer<T: crate::TrunkPlacerType>(&mut self, name: &str, ty: T) {
        self.add_trunk_placer(name, Arc::new(ty));
    }
    /// Register foliage placer type `name`, as [`register_trunk_placer`](Self::register_trunk_placer)
    /// does for `"foliage_placer": {"type": "mymod:..."}`.
    pub fn register_foliage_placer<T: crate::FoliagePlacerType>(&mut self, name: &str, ty: T) {
        self.add_foliage_placer(name, Arc::new(ty));
    }
    /// Register tree decorator type `name`, as [`register_trunk_placer`](Self::register_trunk_placer)
    /// does for an entry of a tree's `"decorators"`.
    pub fn register_tree_decorator<T: crate::TreeDecoratorType>(&mut self, name: &str, ty: T) {
        self.add_tree_decorator(name, Arc::new(ty));
    }
}

/// Registrar operations for tree parts.
pub trait TreePartsRegistrar {
    /// Register trunk placer type `name` with `ty`; mods reach it through `register_trunk_placer`.
    ///
    /// Not on wasm: tree parts are native code that runs on the worldgen worker threads, which a
    /// wasm mod's cannot hand over.
    fn add_trunk_placer(&mut self, name: &str, ty: Arc<dyn crate::TrunkPlacerType>) {
        let _ = ty;
        panic!("trunk placer {name:?}: a wasm mod cannot register a trunk placer type");
    }
    /// Register foliage placer type `name` with `ty`; mods reach it through
    /// `register_foliage_placer`.
    ///
    /// Not on wasm: tree parts are native code that runs on the worldgen worker threads, which a
    /// wasm mod's cannot hand over.
    fn add_foliage_placer(&mut self, name: &str, ty: Arc<dyn crate::FoliagePlacerType>) {
        let _ = ty;
        panic!("foliage placer {name:?}: a wasm mod cannot register a foliage placer type");
    }
    /// Register tree decorator type `name` with `ty`; mods reach it through
    /// `register_tree_decorator`.
    ///
    /// Not on wasm: tree parts are native code that runs on the worldgen worker threads, which a
    /// wasm mod's cannot hand over.
    fn add_tree_decorator(&mut self, name: &str, ty: Arc<dyn crate::TreeDecoratorType>) {
        let _ = ty;
        panic!("tree decorator {name:?}: a wasm mod cannot register a tree decorator type");
    }
}
