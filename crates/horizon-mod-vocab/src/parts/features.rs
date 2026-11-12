use super::*;
/// The feature types registered at startup, for the executable to hand to the engine.
pub(crate) fn installed_feature_type_list() -> &'static [crate::world::feature_type::FeatureTypeInfo]
{
    installed().map_or(&[], |installed| installed.feature_types.as_slice())
}

impl dyn Registrar + '_ {
    /// Register feature type `name` (`spikes` or `mymod:spikes`) once at
    /// startup: a configured feature in a data pack with `"type": "mymod:spikes"`
    /// then places by `ty`, which decodes its JSON and places blocks through a
    /// [`FeatureRegion`](crate::FeatureRegion) with the decorator's random. The
    /// feature runs on the worldgen worker threads. The id is in the calling
    /// mod's namespace; another mod's namespace, the `minecraft` namespace, an
    /// empty name, a name outside vanilla's resource location rules (lowercase
    /// letters, digits and `_ . - /`) and an id registered twice refuse the
    /// mods at startup, naming the mod.
    pub fn register_feature_type<T: crate::FeatureType>(&mut self, name: &str, ty: T) {
        self.add_feature_type(name, Arc::new(ty));
    }
}

/// Registrar operations for feature types.
pub trait FeatureRegistrar {
    /// Register feature type `name` with `ty`; mods reach it through
    /// `register_feature_type`.
    ///
    /// Not on wasm: features are native code that runs on the worldgen worker threads, which a
    /// wasm mod's cannot hand over.
    fn add_feature_type(&mut self, name: &str, ty: Arc<dyn crate::FeatureType>) {
        let _ = ty;
        panic!("feature type {name:?}: a wasm mod cannot register a feature type");
    }
}
