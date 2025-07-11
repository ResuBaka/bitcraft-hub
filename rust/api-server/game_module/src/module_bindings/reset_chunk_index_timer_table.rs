// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::reset_chunk_index_timer_type::ResetChunkIndexTimer;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `reset_chunk_index_timer`.
///
/// Obtain a handle from the [`ResetChunkIndexTimerTableAccess::reset_chunk_index_timer`] method on [`super::RemoteTables`],
/// like `ctx.db.reset_chunk_index_timer()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.reset_chunk_index_timer().on_insert(...)`.
pub struct ResetChunkIndexTimerTableHandle<'ctx> {
    imp: __sdk::TableHandle<ResetChunkIndexTimer>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `reset_chunk_index_timer`.
///
/// Implemented for [`super::RemoteTables`].
pub trait ResetChunkIndexTimerTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`ResetChunkIndexTimerTableHandle`], which mediates access to the table `reset_chunk_index_timer`.
    fn reset_chunk_index_timer(&self) -> ResetChunkIndexTimerTableHandle<'_>;
}

impl ResetChunkIndexTimerTableAccess for super::RemoteTables {
    fn reset_chunk_index_timer(&self) -> ResetChunkIndexTimerTableHandle<'_> {
        ResetChunkIndexTimerTableHandle {
            imp: self
                .imp
                .get_table::<ResetChunkIndexTimer>("reset_chunk_index_timer"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct ResetChunkIndexTimerInsertCallbackId(__sdk::CallbackId);
pub struct ResetChunkIndexTimerDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for ResetChunkIndexTimerTableHandle<'ctx> {
    type Row = ResetChunkIndexTimer;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = ResetChunkIndexTimer> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = ResetChunkIndexTimerInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> ResetChunkIndexTimerInsertCallbackId {
        ResetChunkIndexTimerInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: ResetChunkIndexTimerInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = ResetChunkIndexTimerDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> ResetChunkIndexTimerDeleteCallbackId {
        ResetChunkIndexTimerDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: ResetChunkIndexTimerDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<ResetChunkIndexTimer>("reset_chunk_index_timer");
    _table.add_unique_constraint::<u64>("scheduled_id", |row| &row.scheduled_id);
}
pub struct ResetChunkIndexTimerUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for ResetChunkIndexTimerTableHandle<'ctx> {
    type UpdateCallbackId = ResetChunkIndexTimerUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> ResetChunkIndexTimerUpdateCallbackId {
        ResetChunkIndexTimerUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: ResetChunkIndexTimerUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<ResetChunkIndexTimer>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<ResetChunkIndexTimer>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `scheduled_id` unique index on the table `reset_chunk_index_timer`,
/// which allows point queries on the field of the same name
/// via the [`ResetChunkIndexTimerScheduledIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.reset_chunk_index_timer().scheduled_id().find(...)`.
pub struct ResetChunkIndexTimerScheduledIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<ResetChunkIndexTimer, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> ResetChunkIndexTimerTableHandle<'ctx> {
    /// Get a handle on the `scheduled_id` unique index on the table `reset_chunk_index_timer`.
    pub fn scheduled_id(&self) -> ResetChunkIndexTimerScheduledIdUnique<'ctx> {
        ResetChunkIndexTimerScheduledIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("scheduled_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> ResetChunkIndexTimerScheduledIdUnique<'ctx> {
    /// Find the subscribed row whose `scheduled_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<ResetChunkIndexTimer> {
        self.imp.find(col_val)
    }
}
