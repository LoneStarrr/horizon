//! Host method declarations; access defaults and game forwarders are generated.
#![allow(unused_variables)]
use crate::*;
/// The host's methods both sides share of the `mod_state` area, implemented by the game and reached through [`Host`].
#[horizon_mod_api_macros::host_area]
pub trait ModStateHost {
    /// Side: both.
    /// Capture a SNAPSHOT of the current dimension's attachment residency and values.
    /// Later changes require a fresh query. Removed-but-unswept entities may appear;
    /// writes through stale holders fail. Values are neither cloned nor eagerly decoded.
    #[access(WorldOnly)]
    fn attachment_query(&self, namespace: &str, name: &str) -> AttachmentBatch;
    /// Side: both.
    /// The server-owned holders' attachments; `TYPE.get(host, holder)` reads
    /// one. On a client it is the copy of what the server synced: `set` and
    /// `remove` change only the copy.
    #[access(WorldOnly)]
    fn attachments(&mut self) -> &mut dyn AttachmentStore;
}
/// The server host's methods of the `mod_state` area, implemented by the game and reached through [`ServerHost`].
#[horizon_mod_api_macros::host_area]
pub trait ModStateServerHost {
    /// Side: server.
    /// Validate captured residency against the current dimension immediately before
    /// an ordinary journaled attachment write. This does not open a transaction.
    #[access(WorldOnly)]
    fn attachment_validate_write(
        &self,
        token: AttachmentWriteToken,
    ) -> Result<(), AttachmentRowError>;
    /// Side: server.
    /// Opens a transaction, nested in the open one if any (`Transaction.open`). Only
    /// `host.transaction` reaches it, through its seal.
    #[access(WorldOnly)]
    fn transaction_open(&mut self, seal: TransactionSeal);
    /// Side: server.
    /// `native_transaction_close`, which only `host.transaction` reaches, through its seal.
    #[access(WorldOnly)]
    fn transaction_close(&mut self, seal: TransactionSeal, commit: bool);
}
