//! Plain owned forms for immutable attachment bindings.
use super::*;

/// An immutable binding's transferable state, excluding its query-only decode cache.
#[derive(Clone)]
#[non_exhaustive]
pub enum AttachmentEntryForm {
    /// Saved data, including malformed envelopes, preserved for lazy row decoding.
    Raw(Compound),
    /// The canonical typed binding and its save/sync policy.
    Decoded {
        /// Erased value; incompatible type domains remain a row TypeMismatch.
        value: AttachmentValue,
        /// Stored serialization, if the type has a codec.
        encoded: Option<Compound>,
        /// Copy and sync policy.
        policy: AttachmentPolicy,
    },
}
impl From<&AttachmentEntry> for AttachmentEntryForm {
    fn from(entry: &AttachmentEntry) -> Self {
        match &entry.state {
            Entry::Raw { saved, .. } => Self::Raw(saved.clone()),
            Entry::Decoded(value) => Self::Decoded {
                value: value.value.clone(),
                encoded: value.encoded.clone(),
                policy: value.policy,
            },
        }
    }
}
impl From<AttachmentEntryForm> for AttachmentEntry {
    fn from(form: AttachmentEntryForm) -> Self {
        match form {
            AttachmentEntryForm::Raw(saved) => Self::raw(saved),
            AttachmentEntryForm::Decoded {
                value,
                encoded,
                policy,
            } => Self {
                state: Entry::Decoded(Decoded {
                    value,
                    encoded,
                    policy,
                }),
            },
        }
    }
}
