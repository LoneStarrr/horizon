//! The game with a native mod that adds levels, for `tools/mod_dimensions_e2e.py`. It is what the
//! launcher's generated `main` builds, with one mod: `dimprobe` registers the level
//! `dimprobe:pocket` on the game's noise, the chunk generator `dimprobe:slab`, and the level
//! `dimprobe:slabs` it fills. It also registers the density function type `dimprobe:band`: 1 for y
//! in the `min_y`..=`max_y` of its JSON, else -1, which a data pack's noise router can use. It registers the tree parts `dimprobe:spiral` (trunk
//! placer), `dimprobe:cross` (foliage placer) and `dimprobe:lantern` (tree decorator). The server command `dimprobe enter <level>` sends its player there
//! with `run_command`, inside the command, and `dimprobe later <level>` from the next server tick,
//! where no player's state is swapped in. The run needs a `horizon-native-mods/dimprobe` package
//! beside the executable, which the harness writes.
//!
//! A crate and not an example of horizon-client: an example builds with the dev-dependencies, whose
//! `testing` feature keeps the mods per thread, and the server thread would see none.
use horizon_mod_package::{Side, runtime::BuiltMod};
use mod_api::{
    ChunkAccess, ChunkGenerator, CommandArgumentValue, CommandSource, CustomDensity,
    DensityFunctionType, DensitySampler, DimensionSettings, Flow, FoliageAttachment, FoliagePlacer,
    FoliagePlacerType, Hooks, ModEntry, NativeCommand, RegisterCommandsEvent, Registrar,
    RegistrationContext, RunModCommandEvent, ServerHost, ServerTickEvent, ServerTickPhase, Text,
    TreeAxis, TreeContext, TreeDecorator, TreeDecoratorType, TreeRandom, TrunkPlacer,
    TrunkPlacerType,
};
use std::sync::{Arc, Mutex};

const BUILT: &[BuiltMod] = &[BuiltMod {
    id: "dimprobe",
    version: "1.0.0",
    side: Side::Both,
}];

/// A stone floor up to y=0 under a gold block at each chunk's corner, its y the world's seed
/// modulo 16 above the floor: terrain only this generator makes.
struct Slab;

impl ChunkGenerator for Slab {
    fn build_terrain(&self, chunk: &mut dyn ChunkAccess) {
        let stone = chunk.state("stone");
        let gold = chunk.state("gold_block");
        let [x, z] = [chunk.pos().x * 16, chunk.pos().z * 16];
        for (dx, dz) in (0..16).flat_map(|dx| (0..16).map(move |dz| (dx, dz))) {
            for y in chunk.min_y()..=0 {
                chunk.set_block([x + dx, y, z + dz], stone);
            }
        }
        let marker = 1 + chunk.seed().rem_euclid(16) as i32;
        chunk.set_block([x, marker, z], gold);
    }
}

/// The density type `dimprobe:band`: solid (1) for y in a band, empty (-1) elsewhere.
struct Band;

#[derive(Clone, Copy, Debug)]
struct BandAt([i32; 2]);

impl DensityFunctionType for Band {
    fn decode(&self, config: &serde_json::Value) -> Result<Arc<dyn CustomDensity>, String> {
        let edge = |name: &str| {
            config[name]
                .as_i64()
                .map(|y| y as i32)
                .ok_or(format!("dimprobe:band needs an integer {name}"))
        };
        Ok(Arc::new(BandAt([edge("min_y")?, edge("max_y")?])))
    }
}

impl CustomDensity for BandAt {
    fn range(&self) -> [f32; 2] {
        [-1.0, 1.0]
    }
    /// Only y matters.
    fn domain_axes(&self) -> u8 {
        2
    }
    fn compile(&self, _: i64) -> Arc<dyn DensitySampler> {
        Arc::new(*self)
    }
}

impl DensitySampler for BandAt {
    fn sample_value(&self, [_, y, _]: [i32; 3]) -> f32 {
        if (self.0[0]..=self.0[1]).contains(&y) {
            1.0
        } else {
            -1.0
        }
    }
}

/// The trunk placer `dimprobe:spiral`: a trunk that winds one block around its axis, the
/// footprint at height y being `[(0,0), (1,0), (1,1), (0,1)][y % 4]`. Leaves attach on top.
struct Spiral;

#[derive(Debug)]
struct SpiralTrunk;

impl TrunkPlacerType for Spiral {
    fn decode(&self, _: &serde_json::Value) -> Result<Arc<dyn TrunkPlacer>, String> {
        Ok(Arc::new(SpiralTrunk))
    }
}

impl TrunkPlacer for SpiralTrunk {
    fn place_trunk(
        &self,
        ctx: &mut dyn TreeContext,
        tree_height: i32,
        origin: [i32; 3],
    ) -> Vec<FoliageAttachment> {
        ctx.place_below_trunk([origin[0], origin[1] - 1, origin[2]]);
        for y in 0..tree_height {
            let (dx, dz) = [(0, 0), (1, 0), (1, 1), (0, 1)][y as usize % 4];
            ctx.place_log(
                [origin[0] + dx, origin[1] + y, origin[2] + dz],
                Some(TreeAxis::Y),
            );
        }
        vec![FoliageAttachment {
            pos: [origin[0], origin[1] + tree_height, origin[2]],
            radius_offset: 0,
            double_trunk: false,
        }]
    }
}

/// The foliage placer `dimprobe:cross`: two layers of a plus, its arms `radius` long.
struct Cross;

#[derive(Debug)]
struct CrossFoliage;

impl FoliagePlacerType for Cross {
    fn decode(&self, _: &serde_json::Value) -> Result<Arc<dyn FoliagePlacer>, String> {
        Ok(Arc::new(CrossFoliage))
    }
}

impl FoliagePlacer for CrossFoliage {
    fn foliage_height(&self, _: &mut dyn TreeRandom, _: i32) -> i32 {
        2
    }
    fn create_foliage(
        &self,
        ctx: &mut dyn TreeContext,
        _: i32,
        attachment: FoliageAttachment,
        _: i32,
        radius: i32,
    ) {
        let [x, y, z] = attachment.pos;
        for level in 0..2 {
            for (dx, dz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                ctx.try_place_leaf([x + dx * radius, y - level, z + dz * radius]);
            }
        }
    }
}

/// The tree decorator `dimprobe:lantern`: a lantern beside the lowest log, on the side a draw picks.
struct Lantern;

#[derive(Debug)]
struct HangLantern;

impl TreeDecoratorType for Lantern {
    fn decode(&self, _: &serde_json::Value) -> Result<Arc<dyn TreeDecorator>, String> {
        Ok(Arc::new(HangLantern))
    }
}

impl TreeDecorator for HangLantern {
    fn place(&self, ctx: &mut dyn TreeContext, logs: &[[i32; 3]], _: &[[i32; 3]], _: &[[i32; 3]]) {
        let (Some(&[x, y, z]), Some(lantern)) = (logs.first(), ctx.state("lantern")) else {
            return;
        };
        let [dx, dz] = [[1, 0], [-1, 0], [0, 1], [0, -1]][ctx.next_int_bound(4) as usize];
        ctx.set_decoration([x + dx, y, z + dz], lantern);
    }
}

#[derive(Default)]
struct DimProbe {
    /// The line the next server tick runs.
    later: Mutex<Option<String>>,
}

/// The command that sends `name` to `level`, 40 blocks over the world's y=0.
fn travel(level: &str, name: &str) -> String {
    format!("execute in dimprobe:{level} run tp {name} 8 40 8")
}

#[mod_api::__subscriptions]
impl Hooks for DimProbe {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.register_chunk_generator("slab", Slab);
        r.register_density_function("band", Band);
        r.register_trunk_placer("spiral", Spiral);
        r.register_foliage_placer("cross", Cross);
        r.register_tree_decorator("lantern", Lantern);
        r.register_dimension(
            "pocket",
            DimensionSettings {
                min_y: -128,
                height: 448,
                logical_height: 448,
                ..Default::default()
            },
        );
        r.register_dimension(
            "slabs",
            DimensionSettings {
                min_y: -32,
                height: 128,
                logical_height: 128,
                generator: "dimprobe:slab".into(),
                biome: "minecraft:desert".into(),
                ..Default::default()
            },
        );
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend([
            NativeCommand::new("dimprobe enter <level:word>"),
            NativeCommand::new("dimprobe later <level:word>"),
        ]);
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let (CommandSource::Player { player, .. }, Some(CommandArgumentValue::String(level))) =
            (event.source, event.command.argument("level"))
        else {
            return Flow::Continue;
        };
        let Some(name) = player.name(&*host) else {
            return Flow::Continue;
        };
        let line = travel(level, &name);
        if event.command.literals.iter().any(|word| word == "later") {
            *self.later.lock().unwrap() = Some(line);
            host.commands().feedback(Text::literal("dimprobe queued"));
        } else {
            let ran = host.commands().run_command(&line);
            host.commands()
                .feedback(Text::literal(format!("dimprobe ran={ran}")));
        }
        Flow::Cancel
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(event.phase, ServerTickPhase::ServerTickStart { .. })
            && let Some(line) = self.later.lock().unwrap().take()
        {
            let ran = host.commands().run_command(&line);
            log::info!("dimprobe later ran={ran}");
        }
    }
}

fn main() {
    horizon_client::entry::main_with(
        vec![ModEntry::new("dimprobe", Box::<DimProbe>::default())],
        &[],
        BUILT,
    );
}
