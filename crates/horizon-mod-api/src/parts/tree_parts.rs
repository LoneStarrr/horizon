use super::*;
/// The tree parts registered at startup, for the executable to hand to the engine.
pub(crate) fn installed_tree_part_table() -> Option<&'static crate::tree_parts::TreePartTable> {
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

impl PartRegistrar {
    /// The id `name` registers under as a `kind`, or why the mods are refused. `taken` says
    /// whether the id is already registered as that kind.
    fn tree_part_id(
        &mut self,
        kind: &str,
        name: &str,
        taken: impl Fn(&crate::tree_parts::TreePartTable, &str) -> bool,
    ) -> Option<Box<str>> {
        let (namespace, path) = name.split_once(':').unwrap_or((self.mod_id, name));
        let id = format!("{}:{}", self.mod_id, path);
        let refusal = if namespace != self.mod_id {
            Some(format!(
                "{} registered {kind} {name:?} in the namespace {namespace:?}, which is not its own",
                self.mod_id
            ))
        } else if path.is_empty() {
            Some(format!("{} registered a {kind} with no name", self.mod_id))
        } else if !path
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'/'))
        {
            Some(format!(
                "{} registered {kind} {id:?}, whose name must be lowercase letters, digits and \
                 _ . - / (a resource location path)",
                self.mod_id
            ))
        } else if self.mod_id == "minecraft" {
            Some(format!(
                "{} registered {kind} {id}, in the game's own namespace",
                self.mod_id
            ))
        } else if taken(&self.tree_parts, &id) {
            Some(format!("{} registered {kind} {id} twice", self.mod_id))
        } else {
            None
        };
        match refusal {
            Some(refusal) => {
                self.refused.push(refusal);
                None
            }
            None => Some(id.into()),
        }
    }
}

impl TreePartsRegistrar for PartRegistrar {
    fn add_trunk_placer(&mut self, name: &str, ty: Arc<dyn crate::TrunkPlacerType>) {
        let taken = |t: &crate::tree_parts::TreePartTable, id: &str| {
            t.trunk_placers.iter().any(|(i, _)| **i == *id)
        };
        if let Some(id) = self.tree_part_id("trunk placer", name, taken) {
            let ty = Arc::new(crate::tree_parts::TrunkType(ty));
            self.tree_parts.trunk_placers.push((id, ty));
        }
    }
    fn add_foliage_placer(&mut self, name: &str, ty: Arc<dyn crate::FoliagePlacerType>) {
        let taken = |t: &crate::tree_parts::TreePartTable, id: &str| {
            t.foliage_placers.iter().any(|(i, _)| **i == *id)
        };
        if let Some(id) = self.tree_part_id("foliage placer", name, taken) {
            let ty = Arc::new(crate::tree_parts::FoliageType(ty));
            self.tree_parts.foliage_placers.push((id, ty));
        }
    }
    fn add_tree_decorator(&mut self, name: &str, ty: Arc<dyn crate::TreeDecoratorType>) {
        let taken = |t: &crate::tree_parts::TreePartTable, id: &str| {
            t.decorators.iter().any(|(i, _)| **i == *id)
        };
        if let Some(id) = self.tree_part_id("tree decorator", name, taken) {
            let ty = Arc::new(crate::tree_parts::DecoratorType(ty));
            self.tree_parts.decorators.push((id, ty));
        }
    }
}

/// Fixtures that tests of other crates use to install Parts.
#[cfg(any(test, feature = "testing"))]
pub mod tests {
    #[cfg(test)]
    use super::*;
    #[cfg(test)]
    use crate::*;
    #[cfg(test)]
    use horizon_world::registry::BlockStateId as Engine;
    #[cfg(test)]
    use horizon_world::worldgen::mod_tree_parts::{
        ModTreeContext, ModTreeRandom, TreeAxis as Axis,
    };
    #[cfg(test)]
    use std::collections::{BTreeMap, VecDeque};

    #[cfg(test)]
    const AIR: Engine = Engine(0);
    #[cfg(test)]
    const LOG: Engine = Engine(1);
    #[cfg(test)]
    const LEAF: Engine = Engine(2);
    #[cfg(test)]
    const LANTERN: Engine = Engine(3);
    #[cfg(test)]
    const STONE: Engine = Engine(4);

    /// A small region standing in for a tree being placed: air above y=0, stone at and below.
    /// Draws come from a script, so a test knows which draw each part took.
    #[cfg(test)]
    #[derive(Default)]
    struct Region {
        blocks: BTreeMap<[i32; 3], Engine>,
        draws: VecDeque<i32>,
        drawn: Vec<i32>,
        /// What the config's `offset` draws, in a foliage placer.
        offset: i32,
    }
    #[cfg(test)]
    impl Region {
        fn at(&self, pos: [i32; 3]) -> Engine {
            self.blocks
                .get(&pos)
                .copied()
                .unwrap_or(if pos[1] <= 0 { STONE } else { AIR })
        }
        fn of(&self, state: Engine) -> Vec<[i32; 3]> {
            self.blocks
                .iter()
                .filter(|(_, s)| **s == state)
                .map(|(p, _)| *p)
                .collect()
        }
    }
    #[cfg(test)]
    impl ModTreeRandom for Region {
        fn next_int_bound(&mut self, bound: i32) -> i32 {
            let draw = self.draws.pop_front().expect("a scripted draw") % bound.max(1);
            self.drawn.push(draw);
            draw
        }
        fn next_long(&mut self) -> i64 {
            0
        }
        fn next_float(&mut self) -> f32 {
            0.0
        }
        fn next_double(&mut self) -> f64 {
            0.0
        }
        fn next_boolean(&mut self) -> bool {
            false
        }
    }
    #[cfg(test)]
    impl ModTreeContext for Region {
        fn state(&self, name: &str) -> Option<Engine> {
            match name {
                "oak_log" => Some(LOG),
                "lantern" => Some(LANTERN),
                _ => None,
            }
        }
        fn with_property(&self, state: Engine, _: &str, _: &str) -> Option<Engine> {
            Some(state)
        }
        fn block(&mut self, pos: [i32; 3]) -> Option<Engine> {
            Some(self.at(pos))
        }
        fn is_air(&mut self, pos: [i32; 3]) -> bool {
            self.at(pos) == AIR
        }
        fn valid_tree_pos(&mut self, pos: [i32; 3]) -> bool {
            self.at(pos) == AIR
        }
        fn is_free(&mut self, pos: [i32; 3]) -> bool {
            matches!(self.at(pos), AIR | LOG)
        }
        fn place_log(&mut self, pos: [i32; 3], _: Option<Axis>) -> bool {
            let free = self.at(pos) == AIR;
            if free {
                self.blocks.insert(pos, LOG);
            }
            free
        }
        fn place_log_if_free(&mut self, pos: [i32; 3]) {
            if self.is_free(pos) {
                self.place_log(pos, None);
            }
        }
        fn place_below_trunk(&mut self, _: [i32; 3]) {}
        fn try_place_leaf(&mut self, pos: [i32; 3]) -> bool {
            let free = self.at(pos) == AIR;
            if free {
                self.blocks.insert(pos, LEAF);
            }
            free
        }
        fn foliage_offset(&mut self) -> i32 {
            self.offset
        }
        fn set_decoration(&mut self, pos: [i32; 3], state: Engine) -> bool {
            self.blocks.insert(pos, state);
            true
        }
    }

    /// A trunk that winds one block around its axis: the footprint at height y is
    /// `[(0,0), (1,0), (1,1), (0,1)][y % 4]`, scaled by the config's `width`.
    #[cfg(test)]
    struct Spiral;
    #[cfg(test)]
    #[derive(Debug)]
    struct SpiralTrunk {
        width: i32,
        leaf_reach: i32,
    }
    #[cfg(test)]
    impl mod_api::TrunkPlacerType for Spiral {
        fn decode(
            &self,
            config: &serde_json::Value,
        ) -> Result<Arc<dyn mod_api::TrunkPlacer>, String> {
            Ok(Arc::new(SpiralTrunk {
                width: config["width"].as_i64().ok_or("missing width")? as i32,
                leaf_reach: config["leaf_reach"].as_i64().unwrap_or(0) as i32,
            }))
        }
    }
    #[cfg(test)]
    impl mod_api::TrunkPlacer for SpiralTrunk {
        fn place_trunk(
            &self,
            ctx: &mut dyn mod_api::TreeContext,
            tree_height: i32,
            origin: [i32; 3],
        ) -> Vec<mod_api::FoliageAttachment> {
            let mut top = origin;
            for y in 0..tree_height {
                let (dx, dz) = [(0, 0), (1, 0), (1, 1), (0, 1)][y as usize % 4];
                top = [
                    origin[0] + dx * self.width,
                    origin[1] + y,
                    origin[2] + dz * self.width,
                ];
                ctx.place_log(top, Some(mod_api::TreeAxis::Y));
            }
            vec![mod_api::FoliageAttachment {
                pos: [top[0], top[1] + 1, top[2]],
                radius_offset: self.leaf_reach,
                double_trunk: false,
            }]
        }
    }

    /// A plus-shaped crown: `levels` layers of the attachment and its four neighbours.
    #[cfg(test)]
    struct Cross;
    #[cfg(test)]
    #[derive(Debug)]
    struct CrossFoliage {
        levels: i32,
    }
    #[cfg(test)]
    impl mod_api::FoliagePlacerType for Cross {
        fn decode(
            &self,
            config: &serde_json::Value,
        ) -> Result<Arc<dyn mod_api::FoliagePlacer>, String> {
            let levels = config["levels"].as_i64().ok_or("missing levels")? as i32;
            Ok(Arc::new(CrossFoliage { levels }))
        }
    }
    #[cfg(test)]
    impl mod_api::FoliagePlacer for CrossFoliage {
        fn foliage_height(&self, random: &mut dyn mod_api::TreeRandom, tree_height: i32) -> i32 {
            // One draw, as a vanilla placer with a provider takes.
            self.levels + random.next_int_bound(tree_height)
        }
        fn foliage_radius(&self, random: &mut dyn mod_api::TreeRandom, _: i32, radius: i32) -> i32 {
            radius + random.next_int_bound(2)
        }
        fn create_foliage(
            &self,
            ctx: &mut dyn mod_api::TreeContext,
            _: i32,
            attachment: mod_api::FoliageAttachment,
            _: i32,
            leaf_radius: i32,
        ) {
            let lift = ctx.foliage_offset();
            for level in 0..self.levels {
                let at = [
                    attachment.pos[0],
                    attachment.pos[1] + lift - level,
                    attachment.pos[2],
                ];
                for (dx, dz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                    ctx.try_place_leaf([at[0] + dx * leaf_radius, at[1], at[2] + dz * leaf_radius]);
                }
            }
        }
    }

    /// Hangs a lantern beside the lowest log, on the side its draw picks.
    #[cfg(test)]
    struct Lantern;
    #[cfg(test)]
    #[derive(Debug)]
    struct HangLantern;
    #[cfg(test)]
    impl mod_api::TreeDecoratorType for Lantern {
        fn decode(
            &self,
            config: &serde_json::Value,
        ) -> Result<Arc<dyn mod_api::TreeDecorator>, String> {
            config["block"]
                .as_str()
                .filter(|block| *block == "lantern")
                .map(|_| Arc::new(HangLantern) as Arc<dyn mod_api::TreeDecorator>)
                .ok_or_else(|| "block must be lantern".to_owned())
        }
    }
    #[cfg(test)]
    impl mod_api::TreeDecorator for HangLantern {
        fn place(
            &self,
            ctx: &mut dyn mod_api::TreeContext,
            logs: &[[i32; 3]],
            leaves: &[[i32; 3]],
            _: &[[i32; 3]],
        ) {
            let Some(&[x, y, z]) = logs.first() else {
                return;
            };
            let [dx, dz] = [[1, 0], [-1, 0], [0, 1], [0, -1]][ctx.next_int_bound(4) as usize];
            let lantern = ctx.state("lantern").unwrap();
            ctx.set_decoration([x + dx, y, z + dz], lantern);
            assert!(!leaves.is_empty());
        }
    }

    #[cfg(test)]
    struct All;
    #[cfg(test)]
    #[mod_api::__subscriptions]
    impl Hooks for All {
        fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
            r.register_trunk_placer("spiral_trunk_placer", Spiral);
            r.register_foliage_placer("cross_foliage_placer", Cross);
            r.register_tree_decorator("lantern", Lantern);
        }
    }

    /// A trunk, foliage and decorator the mod registered, decoded from a tree config's own
    /// JSON and run the way the tree feature runs them: height and radius draws first, the
    /// trunk, a foliage placement per attachment, then the decorators on the placed logs
    /// and leaves.
    #[test]
    fn registered_tree_parts_place_a_tree_on_a_small_region() {
        install_parts(&[ModEntry::new("mymod", Box::new(All))]).unwrap();
        let parts = mod_api::__private::installed_tree_parts();
        let trunk = parts.trunk_placers["mymod:spiral_trunk_placer"]
            .decode(&serde_json::json!({
                "type": "mymod:spiral_trunk_placer", "base_height": 5,
                "height_rand_a": 0, "height_rand_b": 0, "width": 1, "leaf_reach": 1}))
            .unwrap();
        let foliage = parts.foliage_placers["mymod:cross_foliage_placer"]
            .decode(&serde_json::json!({
                "type": "mymod:cross_foliage_placer", "radius": 1, "offset": 0, "levels": 2}))
            .unwrap();
        let decorator = parts.decorators["mymod:lantern"]
            .decode(&serde_json::json!({"type": "mymod:lantern", "block": "lantern"}))
            .unwrap();

        // Draws, in order: foliage height (tree_height 5), radius, the lantern's side.
        let mut region = Region {
            draws: [3, 1, 2].into(),
            offset: 0,
            ..Default::default()
        };
        let tree_height = 5;
        let foliage_height = foliage.foliage_height(&mut region, tree_height);
        assert_eq!(foliage_height, 2 + 3);
        let radius = foliage.foliage_radius(&mut region, tree_height - foliage_height, 1);
        assert_eq!(radius, 1 + 1);

        let attachments = trunk.place_trunk(&mut region, tree_height, [0, 1, 0]);
        // y=1..=5 above the stone at y=0: footprints for y offsets 0..5 are
        // (0,0) (1,0) (1,1) (0,1) (0,0).
        let mut logs = region.of(LOG);
        logs.sort_by_key(|p| p[1]);
        assert_eq!(
            logs,
            [[0, 1, 0], [1, 2, 0], [1, 3, 1], [0, 4, 1], [0, 5, 0]]
        );
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].pos, [0, 6, 0]);
        assert_eq!(attachments[0].radius_offset, 1);
        assert!(!attachments[0].double_trunk);

        for attachment in attachments {
            foliage.create_foliage(&mut region, tree_height, attachment, foliage_height, radius);
        }
        // Two layers (y=6 and y=5) of a plus with arms `radius` = 2 long; the log at (0,5,0) stays.
        let mut leaves = region.of(LEAF);
        leaves.sort();
        let mut expected: Vec<[i32; 3]> = [6, 5]
            .iter()
            .flat_map(|y| {
                [[0, 0], [2, 0], [-2, 0], [0, 2], [0, -2]]
                    .into_iter()
                    .map(move |[x, z]| [x, *y, z])
            })
            .filter(|p| *p != [0, 5, 0])
            .collect();
        expected.sort();
        assert_eq!(leaves, expected);

        let mut logs = region.of(LOG);
        logs.sort_by_key(|p| p[1]);
        decorator.place(&mut region, &logs, &leaves, &[]);
        // The lowest log is (0,1,0); draw 2 of the 4 sides is (0,+1).
        assert_eq!(region.of(LANTERN), [[0, 1, 1]]);
        assert_eq!(region.drawn, [3, 1, 2]);
    }

    /// A part's own decode error reaches the tree's load error, and an unknown config is refused.
    #[test]
    fn registered_tree_parts_refuse_their_invalid_configs() {
        install_parts(&[ModEntry::new("mymod", Box::new(All))]).unwrap();
        let parts = mod_api::__private::installed_tree_parts();
        assert_eq!(
            parts.trunk_placers["mymod:spiral_trunk_placer"]
                .decode(&serde_json::json!({}))
                .err()
                .unwrap(),
            "missing width"
        );
        assert_eq!(
            parts.foliage_placers["mymod:cross_foliage_placer"]
                .decode(&serde_json::json!({}))
                .err()
                .unwrap(),
            "missing levels"
        );
        assert_eq!(
            parts.decorators["mymod:lantern"]
                .decode(&serde_json::json!({"block": "x"}))
                .err()
                .unwrap(),
            "block must be lantern"
        );
        assert!(
            !parts
                .trunk_placers
                .contains_key("minecraft:straight_trunk_placer")
        );
    }

    /// Every id a mod may not register, per kind, is refused at startup, naming the mod. A trunk
    /// placer and a foliage placer may share a name: they are separate registries.
    #[test]
    fn invalid_tree_parts_refuse_registration() {
        struct Invalid;
        #[mod_api::__subscriptions]
        impl Hooks for Invalid {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_trunk_placer("spiral", Spiral);
                r.register_foliage_placer("spiral", Cross);
                r.register_trunk_placer("spiral", Spiral);
                r.register_foliage_placer("other:spiral", Cross);
                r.register_tree_decorator("", Lantern);
                r.register_tree_decorator("Bad Name", Lantern);
                r.register_tree_decorator("lantern", Lantern);
                r.register_tree_decorator("lantern", Lantern);
            }
        }
        let errors = install_parts(&[ModEntry::new("test", Box::new(Invalid))]).unwrap_err();
        assert_eq!(errors.len(), 5, "{errors:?}");
        assert!(errors[0].contains("test registered trunk placer test:spiral twice"));
        assert!(errors[1].contains("test registered foliage placer \"other:spiral\""));
        assert!(errors[2].contains("test registered a tree decorator with no name"));
        assert!(errors[3].contains("test registered tree decorator \"test:Bad Name\""));
        assert!(errors[4].contains("test registered tree decorator test:lantern twice"));
    }

    /// The game's own namespace stays the game's.
    #[test]
    fn the_game_namespace_refuses_tree_parts() {
        struct Own;
        #[mod_api::__subscriptions]
        impl Hooks for Own {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                r.register_trunk_placer("straight_trunk_placer", Spiral);
            }
        }
        let errors = install_parts(&[ModEntry::new("minecraft", Box::new(Own))]).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains(
            "minecraft registered trunk placer minecraft:straight_trunk_placer, in the game's own namespace"
        ));
    }
}
