use super::*;
use mod_api::{Hooks, ModEntry, Registrar};

use horizon_world::registry::BlockStateId as Engine;
use horizon_world::worldgen::mod_tree_parts::{ModTreeContext, ModTreeRandom, TreeAxis as Axis};
use std::collections::{BTreeMap, VecDeque};

const AIR: Engine = Engine(0);
const LOG: Engine = Engine(1);
const LEAF: Engine = Engine(2);
const LANTERN: Engine = Engine(3);
const STONE: Engine = Engine(4);

/// A small region standing in for a tree being placed: air above y=0, stone at and below.
/// Draws come from a script, so a test knows which draw each part took.
#[derive(Default)]
struct Region {
    blocks: BTreeMap<[i32; 3], Engine>,
    draws: VecDeque<i32>,
    drawn: Vec<i32>,
    /// What the config's `offset` draws, in a foliage placer.
    offset: i32,
}
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
struct Spiral;
#[derive(Debug)]
struct SpiralTrunk {
    width: i32,
    leaf_reach: i32,
}
impl mod_api::TrunkPlacerType for Spiral {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn mod_api::TrunkPlacer>, String> {
        Ok(Arc::new(SpiralTrunk {
            width: config["width"].as_i64().ok_or("missing width")? as i32,
            leaf_reach: config["leaf_reach"].as_i64().unwrap_or(0) as i32,
        }))
    }
}
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
struct Cross;
#[derive(Debug)]
struct CrossFoliage {
    levels: i32,
}
impl mod_api::FoliagePlacerType for Cross {
    fn decode(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn mod_api::FoliagePlacer>, String> {
        let levels = config["levels"].as_i64().ok_or("missing levels")? as i32;
        Ok(Arc::new(CrossFoliage { levels }))
    }
}
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
struct Lantern;
#[derive(Debug)]
struct HangLantern;
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

struct All;
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
    mod_platform::__private::part_tests::install_parts_fixture(&[ModEntry::new(
        "mymod",
        Box::new(All),
    )])
    .unwrap();
    let parts = installed_tree_parts();
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
    mod_platform::__private::part_tests::install_parts_fixture(&[ModEntry::new(
        "mymod",
        Box::new(All),
    )])
    .unwrap();
    let parts = installed_tree_parts();
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
