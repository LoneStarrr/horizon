use super::*;

mod view_build {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../horizon-mod-internals/build.rs"
    ));

    pub(super) fn render() -> String {
        let _entry: fn() = main;
        generate().unwrap()
    }
}

#[test]
fn root_internals_reach_the_generated_view() {
    const PHASE: &str = "HORIZON_MEMBER_VIEW_TEST";
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../horizon-mod-wasm/tests/bench/internals");
    let child = |phase: &str| {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "wasm::tests::root_internals_reach_the_generated_view",
                "--nocapture",
            ])
            .env(PHASE, phase);
        command
    };
    match std::env::var(PHASE).as_deref() {
        Ok("view") => {
            let view = view_build::render();
            assert!(view.contains(" as Game"), "missing declared Game view");
            assert!(view.contains("HandleChatInputRaw"));
            assert!(view.contains("macro_rules ! raw_listeners"));
            assert!(view.contains("get_many") && view.contains("set_many"));
        }
        Ok("select") => {
            let selected = member_command(&root, "member").unwrap();
            let mut command = child("view");
            for (name, value) in selected.get_envs() {
                if let Some(value) = value {
                    command.env(name, value);
                } else {
                    command.env_remove(name);
                }
            }
            assert!(command.status().unwrap().success());
            let std = member_command(&root, "std").unwrap();
            for name in [
                "HORIZON_MOD_INTERNALS_MANIFEST",
                "HORIZON_MOD_INTERNALS_CATALOG",
            ] {
                assert!(
                    std.get_envs()
                        .any(|(key, value)| key == name && value.is_none())
                );
            }
        }
        _ => {
            // Isolate the build script's environment, as cargo does for each member.
            assert!(
                child("select")
                    .env("HORIZON_MOD_INTERNALS_CATALOG", root.join("internals.json"))
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }
}
