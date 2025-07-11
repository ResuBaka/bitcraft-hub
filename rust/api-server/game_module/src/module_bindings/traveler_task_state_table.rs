// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::traveler_task_state_type::TravelerTaskState;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `traveler_task_state`.
///
/// Obtain a handle from the [`TravelerTaskStateTableAccess::traveler_task_state`] method on [`super::RemoteTables`],
/// like `ctx.db.traveler_task_state()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.traveler_task_state().on_insert(...)`.
pub struct TravelerTaskStateTableHandle<'ctx> {
    imp: __sdk::TableHandle<TravelerTaskState>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `traveler_task_state`.
///
/// Implemented for [`super::RemoteTables`].
pub trait TravelerTaskStateTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`TravelerTaskStateTableHandle`], which mediates access to the table `traveler_task_state`.
    fn traveler_task_state(&self) -> TravelerTaskStateTableHandle<'_>;
}

impl TravelerTaskStateTableAccess for super::RemoteTables {
    fn traveler_task_state(&self) -> TravelerTaskStateTableHandle<'_> {
        TravelerTaskStateTableHandle {
            imp: self
                .imp
                .get_table::<TravelerTaskState>("traveler_task_state"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct TravelerTaskStateInsertCallbackId(__sdk::CallbackId);
pub struct TravelerTaskStateDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for TravelerTaskStateTableHandle<'ctx> {
    type Row = TravelerTaskState;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = TravelerTaskState> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = TravelerTaskStateInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> TravelerTaskStateInsertCallbackId {
        TravelerTaskStateInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: TravelerTaskStateInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = TravelerTaskStateDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> TravelerTaskStateDeleteCallbackId {
        TravelerTaskStateDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: TravelerTaskStateDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<TravelerTaskState>("traveler_task_state");
    _table.add_unique_constraint::<u64>("entity_id", |row| &row.entity_id);
}
pub struct TravelerTaskStateUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for TravelerTaskStateTableHandle<'ctx> {
    type UpdateCallbackId = TravelerTaskStateUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> TravelerTaskStateUpdateCallbackId {
        TravelerTaskStateUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: TravelerTaskStateUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<TravelerTaskState>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<TravelerTaskState>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `entity_id` unique index on the table `traveler_task_state`,
/// which allows point queries on the field of the same name
/// via the [`TravelerTaskStateEntityIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.traveler_task_state().entity_id().find(...)`.
pub struct TravelerTaskStateEntityIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<TravelerTaskState, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> TravelerTaskStateTableHandle<'ctx> {
    /// Get a handle on the `entity_id` unique index on the table `traveler_task_state`.
    pub fn entity_id(&self) -> TravelerTaskStateEntityIdUnique<'ctx> {
        TravelerTaskStateEntityIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("entity_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> TravelerTaskStateEntityIdUnique<'ctx> {
    /// Find the subscribed row whose `entity_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<TravelerTaskState> {
        self.imp.find(col_val)
    }
}
