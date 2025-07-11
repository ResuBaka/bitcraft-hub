// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::knowledge_entry_type::KnowledgeEntry;
use super::knowledge_paving_state_type::KnowledgePavingState;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `knowledge_paving_state`.
///
/// Obtain a handle from the [`KnowledgePavingStateTableAccess::knowledge_paving_state`] method on [`super::RemoteTables`],
/// like `ctx.db.knowledge_paving_state()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.knowledge_paving_state().on_insert(...)`.
pub struct KnowledgePavingStateTableHandle<'ctx> {
    imp: __sdk::TableHandle<KnowledgePavingState>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `knowledge_paving_state`.
///
/// Implemented for [`super::RemoteTables`].
pub trait KnowledgePavingStateTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`KnowledgePavingStateTableHandle`], which mediates access to the table `knowledge_paving_state`.
    fn knowledge_paving_state(&self) -> KnowledgePavingStateTableHandle<'_>;
}

impl KnowledgePavingStateTableAccess for super::RemoteTables {
    fn knowledge_paving_state(&self) -> KnowledgePavingStateTableHandle<'_> {
        KnowledgePavingStateTableHandle {
            imp: self
                .imp
                .get_table::<KnowledgePavingState>("knowledge_paving_state"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct KnowledgePavingStateInsertCallbackId(__sdk::CallbackId);
pub struct KnowledgePavingStateDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for KnowledgePavingStateTableHandle<'ctx> {
    type Row = KnowledgePavingState;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = KnowledgePavingState> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = KnowledgePavingStateInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> KnowledgePavingStateInsertCallbackId {
        KnowledgePavingStateInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: KnowledgePavingStateInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = KnowledgePavingStateDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> KnowledgePavingStateDeleteCallbackId {
        KnowledgePavingStateDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: KnowledgePavingStateDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<KnowledgePavingState>("knowledge_paving_state");
    _table.add_unique_constraint::<u64>("entity_id", |row| &row.entity_id);
}
pub struct KnowledgePavingStateUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for KnowledgePavingStateTableHandle<'ctx> {
    type UpdateCallbackId = KnowledgePavingStateUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> KnowledgePavingStateUpdateCallbackId {
        KnowledgePavingStateUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: KnowledgePavingStateUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<KnowledgePavingState>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<KnowledgePavingState>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `entity_id` unique index on the table `knowledge_paving_state`,
/// which allows point queries on the field of the same name
/// via the [`KnowledgePavingStateEntityIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.knowledge_paving_state().entity_id().find(...)`.
pub struct KnowledgePavingStateEntityIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<KnowledgePavingState, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> KnowledgePavingStateTableHandle<'ctx> {
    /// Get a handle on the `entity_id` unique index on the table `knowledge_paving_state`.
    pub fn entity_id(&self) -> KnowledgePavingStateEntityIdUnique<'ctx> {
        KnowledgePavingStateEntityIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("entity_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> KnowledgePavingStateEntityIdUnique<'ctx> {
    /// Find the subscribed row whose `entity_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<KnowledgePavingState> {
        self.imp.find(col_val)
    }
}
