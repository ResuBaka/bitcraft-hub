// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::dropped_inventory_ownership_timer_type::DroppedInventoryOwnershipTimer;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `dropped_inventory_ownership_timer`.
///
/// Obtain a handle from the [`DroppedInventoryOwnershipTimerTableAccess::dropped_inventory_ownership_timer`] method on [`super::RemoteTables`],
/// like `ctx.db.dropped_inventory_ownership_timer()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.dropped_inventory_ownership_timer().on_insert(...)`.
pub struct DroppedInventoryOwnershipTimerTableHandle<'ctx> {
    imp: __sdk::TableHandle<DroppedInventoryOwnershipTimer>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `dropped_inventory_ownership_timer`.
///
/// Implemented for [`super::RemoteTables`].
pub trait DroppedInventoryOwnershipTimerTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`DroppedInventoryOwnershipTimerTableHandle`], which mediates access to the table `dropped_inventory_ownership_timer`.
    fn dropped_inventory_ownership_timer(&self) -> DroppedInventoryOwnershipTimerTableHandle<'_>;
}

impl DroppedInventoryOwnershipTimerTableAccess for super::RemoteTables {
    fn dropped_inventory_ownership_timer(&self) -> DroppedInventoryOwnershipTimerTableHandle<'_> {
        DroppedInventoryOwnershipTimerTableHandle {
            imp: self
                .imp
                .get_table::<DroppedInventoryOwnershipTimer>("dropped_inventory_ownership_timer"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct DroppedInventoryOwnershipTimerInsertCallbackId(__sdk::CallbackId);
pub struct DroppedInventoryOwnershipTimerDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for DroppedInventoryOwnershipTimerTableHandle<'ctx> {
    type Row = DroppedInventoryOwnershipTimer;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = DroppedInventoryOwnershipTimer> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = DroppedInventoryOwnershipTimerInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> DroppedInventoryOwnershipTimerInsertCallbackId {
        DroppedInventoryOwnershipTimerInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: DroppedInventoryOwnershipTimerInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = DroppedInventoryOwnershipTimerDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> DroppedInventoryOwnershipTimerDeleteCallbackId {
        DroppedInventoryOwnershipTimerDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: DroppedInventoryOwnershipTimerDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache
        .get_or_make_table::<DroppedInventoryOwnershipTimer>("dropped_inventory_ownership_timer");
    _table.add_unique_constraint::<u64>("scheduled_id", |row| &row.scheduled_id);
}
pub struct DroppedInventoryOwnershipTimerUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for DroppedInventoryOwnershipTimerTableHandle<'ctx> {
    type UpdateCallbackId = DroppedInventoryOwnershipTimerUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> DroppedInventoryOwnershipTimerUpdateCallbackId {
        DroppedInventoryOwnershipTimerUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: DroppedInventoryOwnershipTimerUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<DroppedInventoryOwnershipTimer>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse(
            "TableUpdate<DroppedInventoryOwnershipTimer>",
            "TableUpdate",
        )
        .with_cause(e)
        .into()
    })
}

/// Access to the `scheduled_id` unique index on the table `dropped_inventory_ownership_timer`,
/// which allows point queries on the field of the same name
/// via the [`DroppedInventoryOwnershipTimerScheduledIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.dropped_inventory_ownership_timer().scheduled_id().find(...)`.
pub struct DroppedInventoryOwnershipTimerScheduledIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<DroppedInventoryOwnershipTimer, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> DroppedInventoryOwnershipTimerTableHandle<'ctx> {
    /// Get a handle on the `scheduled_id` unique index on the table `dropped_inventory_ownership_timer`.
    pub fn scheduled_id(&self) -> DroppedInventoryOwnershipTimerScheduledIdUnique<'ctx> {
        DroppedInventoryOwnershipTimerScheduledIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("scheduled_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> DroppedInventoryOwnershipTimerScheduledIdUnique<'ctx> {
    /// Find the subscribed row whose `scheduled_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<DroppedInventoryOwnershipTimer> {
        self.imp.find(col_val)
    }
}
