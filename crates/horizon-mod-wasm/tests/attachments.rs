//! Real guest execution of shared batch reads, explicit writes and stale refusal.
use horizon_mod_runtime::attachments::{self as runtime, index::AttachmentIndex};
use mod_api::__private::game::Fields;
use mod_api::{prelude::*, *};
use std::sync::{Arc, Mutex};
struct Server {
    index: Arc<Mutex<AttachmentIndex<AttachmentHolder>>>,
    map: Attachments,
}
fn entity() -> EntityRef {
    EntityRef::new(NativeWorldEpoch::new(1), 7)
}
impl Server {
    fn new() -> Self {
        let mut server = Self {
            index: Default::default(),
            map: Attachments::from_nbt(
                [(
                    "query_fixture".into(),
                    Tag::Compound(
                        [(
                            "value".into(),
                            Tag::Compound(
                                [(
                                    "value".into(),
                                    Tag::Compound([("n".into(), Tag::Int(41))].into()),
                                )]
                                .into(),
                            ),
                        )]
                        .into(),
                    ),
                )]
                .into(),
            ),
        };
        server
            .map
            .bind_index(&server.index, AttachmentHolder::Entity(entity()));
        server
    }
    fn saved(&self) -> i32 {
        let saved = self.map.to_nbt();
        let Tag::Compound(namespace) = &saved[&"query_fixture".into()] else {
            panic!("namespace")
        };
        let Tag::Compound(entry) = &namespace[&"value".into()] else {
            panic!("entry")
        };
        let Tag::Compound(value) = &entry[&"value".into()] else {
            panic!("value")
        };
        let Tag::Int(n) = value[&"n".into()] else {
            panic!("n")
        };
        n
    }
}
impl Host for Server {}
mod_api::impl_host_areas!(Server, except(mod_state));
impl ModStateHost for Server {
    fn attachment_query(&self, namespace: &str, name: &str) -> AttachmentBatch {
        runtime::AttachmentBatch::new(self.index.lock().unwrap().snapshot_named(namespace, name))
            .into()
    }
    fn attachments(&mut self) -> &mut dyn AttachmentStore {
        self
    }
}
impl ServerHost for Server {}
mod_api::impl_server_host_areas!(Server, except(mod_state));
impl ModStateServerHost for Server {
    fn attachment_validate_write(
        &self,
        token: AttachmentWriteToken,
    ) -> Result<(), AttachmentRowError> {
        token.validate(&self.index.lock().unwrap())
    }
}
impl AttachmentHost for Server {
    fn attachments(
        &mut self,
        holder: Holder,
        _: AttachmentAccess,
    ) -> Result<&mut Attachments, AttachmentError> {
        if holder != Holder::Entity(7) {
            return Err(AttachmentError::MissingHolder);
        }
        Ok(&mut self.map)
    }
}
fn chat(hooks: &dyn Hooks, host: &mut Server, text: &str) -> String {
    let mut message = Text::literal("not answered");
    hooks.server_chat(
        host,
        &mut ServerChatEvent {
            player_id: 1,
            player: 1,
            username: "query_test",
            raw_text: text,
            message: &mut message,
            replies: &mut Vec::new(),
        },
    );
    message.plain()
}
#[test]
#[ignore = "needs attachments/mod built with cargo horizon-mod build --wasm and the std member"]
fn generated_batch_crosses_shared_and_mutable_frames() {
    let path = |name| std::path::PathBuf::from(std::env::var(name).expect(name));
    let package = path("HORIZON_WASM_ATTACHMENTS");
    let std = std::fs::read(path("HORIZON_WASM_STD")).unwrap();
    let loaded = horizon_mod_wasm::load(
        &Default::default(),
        &std,
        vec![horizon_mod_wasm::WasmPackage {
            manifest: horizon_mod_package::Package::read(&package)
                .unwrap()
                .manifest,
            module: horizon_mod_package::Package::read_wasm(&package).unwrap(),
        }],
    );
    assert!(loaded.refused.is_empty(), "{:?}", loaded.refused);
    assert_eq!(loaded.mods.len(), 1);
    let hooks = loaded.mods[0].fields().hooks;
    let mut server = Server::new();
    let mut event = EntityMobGriefingEvent {
        entity: entity(),
        can_grief: true,
    };
    hooks.entity_mob_griefing(&server, &mut event);
    assert!(
        !event.can_grief,
        "shared Host query did not complete in the guest"
    );
    assert_eq!(server.saved(), 41, "read query changed stored data");
    assert_eq!(chat(hooks, &mut server, "update"), "updated to 43");
    assert_eq!(
        server.saved(),
        43,
        "guest write did not reach the ordinary store"
    );
    server.map.unbind_index();
    assert!(
        server
            .index
            .lock()
            .unwrap()
            .snapshot_named("query_fixture", "value")
            .is_empty()
    );
    assert_eq!(chat(hooks, &mut server, "stale"), "stale refused");
    assert_eq!(
        server.saved(),
        43,
        "stale row revived or overwrote the value"
    );
}
