//! Real guest -> native -> same guest criterion callbacks, using the generated resource glue.
use super::*;
use crate::{CriterionContext, CriterionTest, CriterionTestError, ItemStack};
use wasm_encoder::reencode::Reencode;
use wasmtime::component::TypedFunc;

/// Start with the complete generated world, replacing only the fixture's executable bodies.
/// Keeping all exports makes these calls use the production `Running` and `WasmHandler`.
fn component(engine: &wasmtime::Engine) -> Component {
    let mut resolve = wit_parser::Resolve::default();
    let package = resolve
        .push_str(
            "mod-api.wit",
            include_str!(concat!(env!("OUT_DIR"), "/mod-api.wit")),
        )
        .unwrap();
    let world = resolve.select_world(&[package], Some("mod")).unwrap();
    // Export anonymous copies, as packaged mods do, so alias types belong to the copy.
    let source = include_str!(concat!(env!("OUT_DIR"), "/mod-api.wit"));
    let mut renamed = source.to_owned();
    for item in resolve.worlds[world].exports.values() {
        let wit_parser::WorldItem::Interface { id, .. } = item else {
            panic!("interface export")
        };
        let name = resolve.interfaces[*id].name.as_ref().unwrap();
        let start = source.find(&format!("interface {name} {{")).unwrap();
        let body = &source[start + format!("interface {name} {{").len()..];
        let body = &body[..body.find("\n}").unwrap()];
        renamed = renamed.replace(
            &format!("  export {name};"),
            &format!("  export fixture-{name}: interface {{{body}\n  }}"),
        );
    }
    let mut resolve = wit_parser::Resolve::default();
    let package = resolve.push_str("fixture-api.wit", &renamed).unwrap();
    let world = resolve.select_world(&[package], Some("mod")).unwrap();
    let fixture = resolve.push_str("fixture.wit", "package test:callback; interface bridge { invoke: func(index: u32) -> u32; } world fixture { import bridge; export run: func(index: u32) -> u32; export trigger: func() -> u32; }").unwrap();
    let fixture = resolve.select_world(&[fixture], Some("fixture")).unwrap();
    for (key, item) in resolve.worlds[fixture].imports.clone() {
        resolve.worlds[world].imports.insert(key, item);
    }
    for (key, item) in resolve.worlds[fixture].exports.clone() {
        resolve.worlds[world].exports.insert(key, item);
    }
    let module =
        wit_component::dummy_module(&resolve, world, wit_parser::ManglingAndAbi::Standard32);
    let mut imports = 0;
    let mut invoke = None;
    let mut conditions = None;
    let mut trigger = None;
    let mut names = HashMap::new();
    for payload in wasmparser::Parser::new(0).parse_all(&module) {
        match payload.unwrap() {
            wasmparser::Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.unwrap();
                    if matches!(import.ty, wasmparser::TypeRef::Func(_)) {
                        if import.name == "invoke" {
                            invoke = Some(imports);
                        }
                        if import.name == "player-trigger-criterion" {
                            trigger = Some(imports);
                        }
                        if import.name == "conditions-json" {
                            conditions = Some(imports);
                        }
                        imports += 1;
                    }
                }
            }
            wasmparser::Payload::ExportSection(section) => {
                for export in section {
                    let export = export.unwrap();
                    if export.kind == wasmparser::ExternalKind::Func {
                        names.insert(export.index, export.name.to_owned());
                    }
                }
            }
            _ => {}
        }
    }
    struct Bodies {
        next: u32,
        names: HashMap<u32, String>,
        invoke: u32,
        conditions: u32,
        trigger: u32,
    }
    impl Reencode for Bodies {
        type Error = std::convert::Infallible;
        fn memory_type(
            &mut self,
            memory: wasmparser::MemoryType,
        ) -> Result<wasm_encoder::MemoryType, wasm_encoder::reencode::Error<Self::Error>> {
            let mut memory = wasm_encoder::reencode::utils::memory_type(self, memory);
            memory.minimum = 1;
            Ok(memory)
        }
        fn parse_function_body(
            &mut self,
            code: &mut wasm_encoder::CodeSection,
            body: wasmparser::FunctionBody<'_>,
        ) -> Result<(), wasm_encoder::reencode::Error<Self::Error>> {
            use wasm_encoder::Instruction as I;
            let name = self.names.get(&self.next).map(String::as_str).unwrap_or("");
            self.next += 1;
            let mut f = wasm_encoder::Function::new([]);
            if name.contains("fixture-block-entity-part")
                && name.ends_with("|block-entity-update-tag")
            {
                // Some(empty NBT bytes), then Some([1, 2, 3]) for the event position.
                for (offset, value) in [1, 0, 0, 1, 1, 2, 3].into_iter().enumerate() {
                    f.instruction(&I::I32Const(4096 + offset as i32 * 4))
                        .instruction(&I::I32Const(value))
                        .instruction(&I::I32Store(wasm_encoder::MemArg {
                            offset: 0,
                            align: 2,
                            memory_index: 0,
                        }));
                }
                f.instruction(&I::I32Const(4096));
            } else if name.ends_with("|run") {
                f.instruction(&I::LocalGet(0))
                    .instruction(&I::Call(self.invoke));
            } else if name.ends_with("|trigger") {
                let mem = wasm_encoder::MemArg {
                    offset: 0,
                    align: 0,
                    memory_index: 0,
                };
                f.instruction(&I::I32Const(4096))
                    .instruction(&I::I64Const(i64::from_le_bytes(*b"test:pro")))
                    .instruction(&I::I64Store(mem));
                f.instruction(&I::I32Const(4104))
                    .instruction(&I::I32Const(i32::from(u16::from_le_bytes(*b"be"))))
                    .instruction(&I::I32Store16(mem));
                // Player epoch/id/uuid, trigger string, handler index/subscriptions/held, result area.
                f.instruction(&I::I64Const(1))
                    .instruction(&I::I32Const(1))
                    .instruction(&I::I64Const(0))
                    .instruction(&I::I64Const(1));
                for value in [4096, 10, 0, 0, 0, 0, 96] {
                    f.instruction(&I::I32Const(value));
                }
                f.instruction(&I::Call(self.trigger))
                    .instruction(&I::I32Const(96))
                    .instruction(&I::I32Load8U(mem))
                    .instruction(&I::I32Eqz);
            } else if name.contains("fixture-criterion-test") && name.ends_with("|test") {
                // A nested callback must restore this predicate's criterion context.
                f.instruction(&I::LocalGet(0))
                    .instruction(&I::I32Const(4))
                    .instruction(&I::I32Eq)
                    .instruction(&I::If(wasm_encoder::BlockType::Empty));
                f.instruction(&I::I32Const(5))
                    .instruction(&I::Call(self.invoke))
                    .instruction(&I::Drop)
                    .instruction(&I::End);
                // Index 1 exhausts the outer budget; index 3 traps explicitly.
                f.instruction(&I::LocalGet(0))
                    .instruction(&I::I32Const(1))
                    .instruction(&I::I32Eq)
                    .instruction(&I::If(wasm_encoder::BlockType::Empty));
                f.instruction(&I::Loop(wasm_encoder::BlockType::Empty))
                    .instruction(&I::Br(0))
                    .instruction(&I::End)
                    .instruction(&I::End);
                f.instruction(&I::LocalGet(0))
                    .instruction(&I::I32Const(3))
                    .instruction(&I::I32Eq)
                    .instruction(&I::If(wasm_encoder::BlockType::Empty))
                    .instruction(&I::Unreachable)
                    .instruction(&I::End);
                f.instruction(&I::I32Const(32))
                    .instruction(&I::Call(self.conditions));
                // Result<bool, CriterionTestError>: success tag, then the bool payload.
                let byte = wasm_encoder::MemArg {
                    offset: 0,
                    align: 0,
                    memory_index: 0,
                };
                f.instruction(&I::I32Const(64))
                    .instruction(&I::I32Const(0))
                    .instruction(&I::I32Store8(byte));
                f.instruction(&I::I32Const(65));
                f.instruction(&I::LocalGet(0))
                    .instruction(&I::I32Const(2))
                    .instruction(&I::I32Eq)
                    .instruction(&I::If(wasm_encoder::BlockType::Result(
                        wasm_encoder::ValType::I32,
                    )));
                f.instruction(&I::I32Const(1))
                    .instruction(&I::MemoryGrow(0))
                    .instruction(&I::I32Const(-1))
                    .instruction(&I::I32Eq);
                f.instruction(&I::Else)
                    .instruction(&I::I32Const(32))
                    .instruction(&I::I32Load(byte))
                    .instruction(&I::I32Load8U(byte))
                    .instruction(&I::I32Const(i32::from(b'{')))
                    .instruction(&I::I32Eq)
                    .instruction(&I::End);
                f.instruction(&I::I32Store8(byte))
                    .instruction(&I::I32Const(64));
            } else if name.contains("realloc") {
                f.instruction(&I::I32Const(8192));
            } else {
                return wasm_encoder::reencode::utils::parse_function_body(self, code, body);
            }
            f.instruction(&I::End);
            code.function(&f);
            Ok(())
        }
    }
    let mut rewritten = wasm_encoder::Module::new();
    Bodies {
        next: imports,
        names,
        invoke: invoke.unwrap(),
        conditions: conditions.unwrap(),
        trigger: trigger.unwrap(),
    }
    .parse_core_module(&mut rewritten, wasmparser::Parser::new(0), &module)
    .unwrap();
    let mut module = rewritten.finish();
    wit_component::embed_component_metadata(
        &mut module,
        &resolve,
        world,
        wit_component::StringEncoding::UTF8,
        true,
    )
    .unwrap();
    let bytes = wit_component::ComponentEncoder::default()
        .module(&module)
        .unwrap()
        .encode()
        .unwrap();
    Component::new(engine, bytes).unwrap()
}

struct Context(&'static str);
impl CriterionContext for Context {
    fn conditions_json(&self) -> String {
        self.0.into()
    }
    fn item(&self, _: &str, _: &ItemStack) -> bool {
        unreachable!()
    }
}

fn fixture() -> (Arc<Running>, TypedFunc<(u32,), (u32,)>) {
    fixture_with_budget(true)
}

fn fixture_with_budget(fuel: bool) -> (Arc<Running>, TypedFunc<(u32,), (u32,)>) {
    let mut config = wasmtime::Config::new();
    config.consume_fuel(fuel).epoch_interruption(!fuel);
    let engine = wasmtime::Engine::new(&config).unwrap();
    let component = component(&engine);
    let mut linker = Linker::<State>::new(&engine);
    glue::Mod::add_to_linker::<State, wasmtime::component::HasSelf<State>>(&mut linker, |s| s)
        .unwrap();
    glue::add_store_imports(&mut linker).unwrap();
    linker
        .instance("test:callback/bridge")
        .unwrap()
        .func_wrap("invoke", |store, (index,): (u32,)| {
            if index == 7 {
                store.engine().increment_epoch();
            }
            with_callback_store(store, || {
                let module = CALLER.take().unwrap();
                CALLER.set(Some(module.clone()));
                let outer = CURRENT.get();
                let handler = WasmHandler {
                    module,
                    index,
                    subscribed: &[],
                    erased: true,
                };
                let answer = handler.test(&Context(if index == 5 {
                    "null"
                } else {
                    "{\"match\":true}"
                }));
                assert_eq!(
                    CURRENT.get(),
                    outer,
                    "the callback restored its outer frame"
                );
                assert!(
                    handler.module.group.store.try_lock().is_err(),
                    "the callback kept the outer group lock"
                );
                assert!(
                    !handler.module.call_or(
                        Frame::default(),
                        Call::Hook,
                        || false,
                        |_, _| Ok(true)
                    ),
                    "a predicate scope must not enable recursive hooks"
                );
                Ok((match answer {
                    Ok(false) => 0u32,
                    Ok(true) => 1,
                    Err(CriterionTestError::Unavailable) => 2,
                },))
            })
        })
        .unwrap();
    let budgets = Budgets {
        fuel: fuel.then_some(Fuel {
            call: 10_000,
            load: 10_000,
        }),
        memory: 65_536,
        wait: Duration::ZERO,
        ..Budgets::default()
    };
    let mut store = Store::new(
        &engine,
        State {
            limits: wasmtime::StoreLimitsBuilder::new()
                .memory_size(budgets.memory)
                .build(),
            buffers: HashMap::new(),
        },
    );
    store.limiter(|state| &mut state.limits);
    budgets.arm(&mut store, Call::Load);
    let instance = linker.instantiate(&mut store, &component).unwrap();
    let run = instance.get_typed_func(&mut store, "run").unwrap();
    let exports = glue::ModExports::find(&mut store, &instance, "fixture").unwrap();
    let module = Arc::new(Running {
        group: Arc::new(Group::new(store, budgets)),
        instance,
        exports,
        id: "callback_fixture",
        released: Mutex::default(),
    });
    (module, run)
}

#[test]
fn malformed_method_return_does_not_apply_a_valid_out_event() {
    let (module, _) = fixture();
    let handler = WasmHandler {
        module,
        index: 0,
        subscribed: &[],
        erased: false,
    };
    let entity = crate::ModBlockEntity::new("test:counter".into(), Default::default());
    let revision = entity.revision();
    let mut event = crate::BlockEntityUpdateTagEvent {
        pos: [0; 3],
        entity: &entity,
    };
    assert!(crate::BlockEntityPart::block_entity_update_tag(&handler, &mut event).is_none());
    assert_eq!(event.pos, [0; 3]);
    assert!(entity.data.is_empty());
    assert_eq!(entity.revision(), revision);
}

fn run(module: &Arc<Running>, function: TypedFunc<(u32,), (u32,)>, index: u32) -> Option<u32> {
    module.call_or(
        Frame::default(),
        Call::Hook,
        || None,
        |_, store| Ok(Some(function.call(store, (index,))?.0)),
    )
}

#[test]
fn criterion_callbacks_reenter_read_their_context_and_restore_nested_frames() {
    let (module, function) = fixture();
    for index in [0, 5, 4, 0] {
        assert_eq!(run(&module, function, index), Some(u32::from(index != 5)));
    }
    assert!(!module.group.poisoned.load(Ordering::Relaxed));
    assert!(CURRENT.get().is_null());
    assert!(CALLBACK_STORE.get().is_null());
    assert!(
        module.group.enter(Call::Hook).is_ok(),
        "the outer lock was released"
    );
}

#[test]
fn callback_memory_growth_keeps_the_group_limit() {
    let (module, function) = fixture();
    assert_eq!(run(&module, function, 2), Some(1));
    assert_eq!(run(&module, function, 0), Some(1));
}

#[test]
fn callback_fuel_exhaustion_and_traps_stop_the_group_and_release_scopes() {
    for index in [1, 3] {
        let (module, function) = fixture();
        let _ = run(&module, function, index);
        assert!(module.group.poisoned.load(Ordering::Relaxed));
        assert_eq!(run(&module, function, 0), None);
        assert!(CURRENT.get().is_null());
        assert!(CALLBACK_STORE.get().is_null());
        assert_eq!(
            module.group.enter(Call::Hook).err(),
            Some(CallRefused::Stopped)
        );
    }
}

#[test]
fn callbacks_share_remaining_fuel_instead_of_refilling_it() {
    let (module, function) = fixture();
    let remaining = module
        .call_or(
            Frame::default(),
            Call::Hook,
            || None,
            |_, store| {
                store.set_fuel(500)?;
                assert_eq!(function.call(&mut *store, (0,))?.0, 1);
                Ok(Some(store.get_fuel()?))
            },
        )
        .unwrap();
    assert!(
        remaining > 0 && remaining < 500,
        "remaining fuel: {remaining}"
    );
}

#[test]
fn callbacks_keep_the_outer_epoch_deadline() {
    let (module, function) = fixture_with_budget(false);
    let _ = module.call_or(
        Frame::default(),
        Call::Hook,
        || None,
        |_, store| {
            store.set_epoch_deadline(1);
            Ok(Some(function.call(store, (7,))?.0))
        },
    );
    assert!(module.group.poisoned.load(Ordering::Relaxed));
    assert_eq!(run(&module, function, 0), None);
}

#[test]
fn generated_trigger_import_runs_its_wasm_predicate_synchronously() {
    struct Host {
        tested: usize,
    }
    impl crate::Host for Host {}
    mod_api::impl_host_areas!(Host);
    impl crate::ServerHost for Host {
        fn backend_mut(&mut self) -> Option<crate::BackendMutRef<'_>> {
            Some(crate::BackendMutRef::of(self))
        }
    }
    mod_api::impl_server_host_areas!(Host);
    impl crate::BackendMut for Host {}
    mod_api::impl_backend_mut_areas!(Host, except(players));
    impl crate::prelude::PlayersBackendMut for Host {
        fn player_trigger_criterion(
            &mut self,
            player: crate::PlayerRef,
            trigger: &crate::Identifier,
            test: Arc<dyn CriterionTest>,
        ) -> Result<(), crate::TriggerCriterionError> {
            assert_eq!(player.uuid, 1);
            assert_eq!(trigger.to_string(), "test:probe");
            assert_eq!(test.test(&Context("{\"match\":true}")), Ok(true));
            self.tested += 1;
            Ok(())
        }
    }
    let (module, _) = fixture();
    let trigger = module
        .instance
        .get_typed_func::<(), (u32,)>(&mut *module.group.store.lock().unwrap(), "trigger")
        .unwrap();
    let mut host = Host { tested: 0 };
    let mut frame = Frame::default();
    frame.server_host_mut(&mut host);
    let answer = module.call_or(
        frame,
        Call::Hook,
        || None,
        |_, store| Ok(Some(trigger.call(store, ())?.0)),
    );
    assert_eq!(answer, Some(1));
    assert_eq!(host.tested, 1);
    assert!(!module.group.poisoned.load(Ordering::Relaxed));
}

#[test]
fn callback_scopes_are_removed_on_native_unwind() {
    let (module, _) = fixture();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        module.call_or(
            Frame::default(),
            Call::Hook,
            || (),
            |_, store| {
                with_callback_store(store.as_context_mut(), || {
                    let mut loan = ResumedStore::take(&module.group).unwrap();
                    assert!(loan.context().get_fuel().is_ok());
                    panic!("fixture native unwind");
                })
            },
        );
    }));
    assert!(result.is_err());
    assert!(CURRENT.get().is_null());
    assert!(CALLBACK_STORE.get().is_null());
    assert!(module.group.enter(Call::Hook).is_ok());
}
