//! A real guest exercising the generated whole-batch boundary on shared and mutable frames.
use mod_api::*;
use std::sync::Mutex;
static VALUE: AttachmentType<u32> = AttachmentType {
    namespace: "query_fixture",
    name: "value",
    default: || panic!("no defaults"),
    codec: Some(AttachmentCodec {
        encode: |value| Ok([("n".into(), Tag::Int(*value as i32))].into()),
        decode: |saved| match saved.get(&"n".into()) {
            Some(Tag::Int(n)) => Ok(*n as u32),
            _ => Err(AttachmentError::Decode),
        },
    }),
    copy_on_death: false,
    sync: false,
};
static CAPTURED: Mutex<Option<AttachmentQueryMut<'static, u32>>> = Mutex::new(None);
#[derive(Default)]
pub struct Mod;
fn total(host: &dyn Host) -> u32 {
    host.query(&VALUE)
        .unwrap()
        .iter()
        .map(|row| *row.unwrap().value)
        .sum()
}
#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn entity_mob_griefing(&self, host: &dyn Host, event: &mut EntityMobGriefingEvent) {
        assert_eq!(total(host), 41);
        event.can_grief = false;
    }
    fn server_chat(&self, host: &mut dyn ServerHost, event: &mut ServerChatEvent<'_>) -> Flow {
        match event.raw_text {
            "update" => {
                let batch = host.query_mut(&VALUE).unwrap();
                let row = batch.iter().next().unwrap().unwrap();
                assert_eq!(*row.value(), 41);
                row.set(host, 42).unwrap();
                assert_eq!(total(host), 42);
                assert_eq!(*row.value(), 41);
                assert_eq!(row.remove(host).unwrap(), Some(42));
                assert_eq!(total(host), 0);
                // Detaching a value does not stale the still-loaded holder.
                row.set(host, 43).unwrap();
                assert_eq!(total(host), 43);
                *CAPTURED.lock().unwrap() = Some(host.query_mut(&VALUE).unwrap());
                *event.message = Text::literal("updated to 43");
            }
            "stale" => {
                let captured = CAPTURED.lock().unwrap();
                let row = captured.as_ref().unwrap().iter().next().unwrap().unwrap();
                assert_eq!(row.set(host, 99), Err(AttachmentRowError::StaleHolder));
                assert_eq!(total(host), 0);
                *event.message = Text::literal("stale refused");
            }
            _ => panic!("unexpected fixture command"),
        }
        Flow::Continue
    }
}
