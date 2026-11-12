use mod_api::{Hooks, MenuLinksEvent, RecipeSourcesEvent, ServerHost};

pub struct Lender;

#[mod_api::__subscriptions]
impl Hooks for Lender {
    fn recipe_sources(&self, _: &mut dyn ServerHost, event: &mut RecipeSourcesEvent<'_>) {
        assert_eq!(event.player, 0x1234_5678_9abc_def0_1234_5678_9abc_def0);
        assert_eq!(event.menu, "wasm_fixture:machine");
        assert_eq!(event.pos, [-17, 64, 32]);
        assert_eq!(event.sources.as_slice(), [[-17, 63, 32]]);
        event.sources.push([-16, 64, 32]);
        event.sources.push([-18, 64, 32]);
    }

    fn menu_links(&self, _: &mut dyn ServerHost, event: &mut MenuLinksEvent<'_>) {
        assert_eq!(event.player, 0x1234_5678_9abc_def0_1234_5678_9abc_def0);
        assert_eq!(event.menu, "wasm_fixture:machine");
        assert_eq!(event.pos, [-17, 64, 32]);
        assert_eq!(event.links.as_slice(), [[-17, 63, 32]]);
        event.links.push([-16, 64, 32]);
        event.links.push([-18, 64, 32]);
    }
}

pub fn check(host: &mut dyn ServerHost) {
    let mut sources = vec![[-17, 63, 32]];
    mod_api::dispatch_recipe_sources(
        host,
        0x1234_5678_9abc_def0_1234_5678_9abc_def0,
        "wasm_fixture:machine",
        [-17, 64, 32],
        &mut sources,
    );
    assert_eq!(sources, [[-17, 63, 32], [-16, 64, 32], [-18, 64, 32]]);
    let mut links = vec![[-17, 63, 32]];
    mod_api::dispatch_menu_links(
        host,
        0x1234_5678_9abc_def0_1234_5678_9abc_def0,
        "wasm_fixture:machine",
        [-17, 64, 32],
        &mut links,
    );
    assert_eq!(links, sources);
}
