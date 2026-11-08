//! Owned snapshot data shared by native and generated component boundaries.
use crate::*;
use horizon_mod_runtime::attachments::{self as runtime, index};
use std::sync::Arc;

/// A SNAPSHOT of membership and value bindings in the host's current dimension.
/// Later changes require a fresh query. This is a residency view: removed but
/// unswept entities may appear, and stale writes fail. Native acquisition neither
/// clones values nor decodes saved payloads; iteration decodes lazily.
#[derive(Clone)]
#[non_exhaustive]
#[horizon_mod_api_macros::wasm(via = Vec<AttachmentRowForm>)]
pub struct AttachmentBatch(runtime::AttachmentBatch<AttachmentHolder>);
impl From<runtime::AttachmentBatch<AttachmentHolder>> for AttachmentBatch {
    fn from(batch: runtime::AttachmentBatch<AttachmentHolder>) -> Self {
        Self(batch)
    }
}
impl From<&AttachmentBatch> for Vec<AttachmentRowForm> {
    fn from(batch: &AttachmentBatch) -> Self {
        batch
            .0
            .rows()
            .iter()
            .map(|row| {
                let form = index::AttachmentRowForm::from(row);
                AttachmentRowForm {
                    token: AttachmentOwnerForm {
                        holder: form.token.holder,
                        generation: form.token.generation,
                    },
                    entry: form.entry,
                }
            })
            .collect()
    }
}
impl From<Vec<AttachmentRowForm>> for AttachmentBatch {
    fn from(rows: Vec<AttachmentRowForm>) -> Self {
        if rows.is_empty() {
            return Self(runtime::AttachmentBatch::empty());
        }
        Self(runtime::AttachmentBatch::new(Arc::new(
            rows.into_iter()
                .map(|row| {
                    let token = index::AttachmentOwnerForm::from(row.token);
                    index::AttachmentRowForm::from_parts(token, row.entry).into()
                })
                .collect(),
        )))
    }
}
impl AttachmentBatch {
    /// Bind a type without eagerly decoding or cloning any value.
    /// This view borrows only the key, never the host.
    pub fn query<T: Send + Sync + 'static>(
        self,
        key: &AttachmentType<T>,
    ) -> Result<AttachmentQuery<'_, T>, AttachmentError> {
        self.0.query(key)
    }
}
/// A typed snapshot whose iterator borrows values from its captured bindings.
pub type AttachmentQuery<'key, T> = runtime::AttachmentQuery<'key, AttachmentHolder, T>;

/// A private-field scoped residency identity, not an access authorization token.
#[derive(Clone)]
#[non_exhaustive]
#[horizon_mod_api_macros::wasm(via = AttachmentOwnerForm)]
pub struct AttachmentWriteToken(index::AttachmentOwner<AttachmentHolder>);
impl From<index::AttachmentOwner<AttachmentHolder>> for AttachmentWriteToken {
    fn from(token: index::AttachmentOwner<AttachmentHolder>) -> Self {
        Self(token)
    }
}
impl From<&AttachmentWriteToken> for AttachmentOwnerForm {
    fn from(token: &AttachmentWriteToken) -> Self {
        let form = index::AttachmentOwnerForm::from(&token.0);
        Self {
            holder: form.holder,
            generation: form.generation,
        }
    }
}
impl From<AttachmentOwnerForm> for AttachmentWriteToken {
    fn from(form: AttachmentOwnerForm) -> Self {
        Self(index::AttachmentOwnerForm::from(form).into())
    }
}
impl From<AttachmentOwnerForm> for index::AttachmentOwnerForm<AttachmentHolder> {
    fn from(form: AttachmentOwnerForm) -> Self {
        Self::from_parts(form.holder, form.generation)
    }
}

/// Plain representation of a scoped residency token.
#[derive(Clone)]
#[non_exhaustive]
pub struct AttachmentOwnerForm {
    /// Captured scoped holder.
    pub holder: AttachmentHolder,
    /// Registration generation, checked against the current index before writing.
    pub generation: u64,
}
/// Plain representation of one captured row.
#[derive(Clone)]
#[non_exhaustive]
pub struct AttachmentRowForm {
    /// Captured residency identity.
    pub token: AttachmentOwnerForm,
    /// Shared immutable value binding.
    pub entry: Arc<AttachmentEntry>,
}
impl AttachmentWriteToken {
    /// Check the current dimension's residency before the ordinary write path.
    // hidden: the owning engine validates tokens; mods use the row's set/remove methods.
    #[doc(hidden)]
    pub fn validate(
        &self,
        index: &index::AttachmentIndex<AttachmentHolder>,
    ) -> Result<(), AttachmentRowError> {
        index.validate_owner(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static KEY: AttachmentType<u32> = AttachmentType {
        namespace: "test",
        name: "value",
        default: || 0,
        codec: None,
        copy_on_death: false,
        sync: false,
    };
    #[test]
    fn batch_forms_preserve_values_and_stale_residency() {
        let holder = AttachmentHolder::Entity(EntityRef::new(NativeWorldEpoch::new(1), 42));
        let entry = Arc::new(AttachmentEntry::from(AttachmentEntryForm::Decoded {
            value: Arc::new(7u32),
            encoded: None,
            policy: AttachmentPolicy {
                copy_on_death: false,
                sync: false,
            },
        }));
        let key = ("test".into(), "value".into());
        let mut index = index::AttachmentIndex::default();
        index.register(holder, [(key, entry.clone())]).into_result();
        let batch = AttachmentBatch::from(runtime::AttachmentBatch::new(
            index.snapshot_named("test", "value"),
        ));
        let forms = Vec::from(&batch);
        let token = AttachmentWriteToken::from(forms[0].token.clone());
        assert_eq!(token.validate(&index), Ok(()));
        let restored = AttachmentBatch::from(forms).query(&KEY).unwrap();
        let row = restored.iter().next().unwrap().unwrap();
        assert_eq!(row.holder, holder);
        assert_eq!(*row.value, 7);
        assert_eq!(
            token.validate(&index::AttachmentIndex::default()),
            Err(AttachmentRowError::StaleHolder)
        );
        index.unload(holder).into_result();
        assert_eq!(token.validate(&index), Err(AttachmentRowError::StaleHolder));
        index
            .register(holder, [(("test".into(), "value".into()), entry)])
            .into_result();
        assert_eq!(token.validate(&index), Err(AttachmentRowError::StaleHolder));
        assert_eq!(*row.value, 7);
    }
    #[test]
    fn entry_forms_preserve_malformed_saved_and_type_errors() {
        let malformed = AttachmentEntry::from(AttachmentEntryForm::Raw(Compound::new()));
        let restored = AttachmentEntry::from(AttachmentEntryForm::from(&malformed));
        assert_eq!(restored.read(&KEY), Err(AttachmentError::Decode));
        let mismatch = AttachmentEntry::from(AttachmentEntryForm::Decoded {
            value: Arc::new("other type"),
            encoded: Some([("payload".into(), Tag::Int(37))].into()),
            policy: AttachmentPolicy {
                copy_on_death: true,
                sync: true,
            },
        });
        let restored = AttachmentEntry::from(AttachmentEntryForm::from(&mismatch));
        assert_eq!(restored.read(&KEY), Err(AttachmentError::TypeMismatch));
        let AttachmentEntryForm::Decoded {
            encoded, policy, ..
        } = AttachmentEntryForm::from(&restored)
        else {
            panic!("decoded entry lost its form");
        };
        assert_eq!(encoded, Some([("payload".into(), Tag::Int(37))].into()));
        assert!(policy.copy_on_death);
        assert!(policy.sync);
    }
}
