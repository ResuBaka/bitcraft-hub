// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::teleportation_energy_regen_loop_timer_type::TeleportationEnergyRegenLoopTimer;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `teleportation_energy_regen_loop_timer`.
///
/// Obtain a handle from the [`TeleportationEnergyRegenLoopTimerTableAccess::teleportation_energy_regen_loop_timer`] method on [`super::RemoteTables`],
/// like `ctx.db.teleportation_energy_regen_loop_timer()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.teleportation_energy_regen_loop_timer().on_insert(...)`.
pub struct TeleportationEnergyRegenLoopTimerTableHandle<'ctx> {
    imp: __sdk::TableHandle<TeleportationEnergyRegenLoopTimer>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `teleportation_energy_regen_loop_timer`.
///
/// Implemented for [`super::RemoteTables`].
pub trait TeleportationEnergyRegenLoopTimerTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`TeleportationEnergyRegenLoopTimerTableHandle`], which mediates access to the table `teleportation_energy_regen_loop_timer`.
    fn teleportation_energy_regen_loop_timer(
        &self,
    ) -> TeleportationEnergyRegenLoopTimerTableHandle<'_>;
}

impl TeleportationEnergyRegenLoopTimerTableAccess for super::RemoteTables {
    fn teleportation_energy_regen_loop_timer(
        &self,
    ) -> TeleportationEnergyRegenLoopTimerTableHandle<'_> {
        TeleportationEnergyRegenLoopTimerTableHandle {
            imp: self.imp.get_table::<TeleportationEnergyRegenLoopTimer>(
                "teleportation_energy_regen_loop_timer",
            ),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct TeleportationEnergyRegenLoopTimerInsertCallbackId(__sdk::CallbackId);
pub struct TeleportationEnergyRegenLoopTimerDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for TeleportationEnergyRegenLoopTimerTableHandle<'ctx> {
    type Row = TeleportationEnergyRegenLoopTimer;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = TeleportationEnergyRegenLoopTimer> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = TeleportationEnergyRegenLoopTimerInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> TeleportationEnergyRegenLoopTimerInsertCallbackId {
        TeleportationEnergyRegenLoopTimerInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: TeleportationEnergyRegenLoopTimerInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = TeleportationEnergyRegenLoopTimerDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> TeleportationEnergyRegenLoopTimerDeleteCallbackId {
        TeleportationEnergyRegenLoopTimerDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: TeleportationEnergyRegenLoopTimerDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<TeleportationEnergyRegenLoopTimer>(
        "teleportation_energy_regen_loop_timer",
    );
    _table.add_unique_constraint::<u64>("scheduled_id", |row| &row.scheduled_id);
}
pub struct TeleportationEnergyRegenLoopTimerUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for TeleportationEnergyRegenLoopTimerTableHandle<'ctx> {
    type UpdateCallbackId = TeleportationEnergyRegenLoopTimerUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> TeleportationEnergyRegenLoopTimerUpdateCallbackId {
        TeleportationEnergyRegenLoopTimerUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: TeleportationEnergyRegenLoopTimerUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<TeleportationEnergyRegenLoopTimer>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse(
            "TableUpdate<TeleportationEnergyRegenLoopTimer>",
            "TableUpdate",
        )
        .with_cause(e)
        .into()
    })
}

/// Access to the `scheduled_id` unique index on the table `teleportation_energy_regen_loop_timer`,
/// which allows point queries on the field of the same name
/// via the [`TeleportationEnergyRegenLoopTimerScheduledIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.teleportation_energy_regen_loop_timer().scheduled_id().find(...)`.
pub struct TeleportationEnergyRegenLoopTimerScheduledIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<TeleportationEnergyRegenLoopTimer, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> TeleportationEnergyRegenLoopTimerTableHandle<'ctx> {
    /// Get a handle on the `scheduled_id` unique index on the table `teleportation_energy_regen_loop_timer`.
    pub fn scheduled_id(&self) -> TeleportationEnergyRegenLoopTimerScheduledIdUnique<'ctx> {
        TeleportationEnergyRegenLoopTimerScheduledIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("scheduled_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> TeleportationEnergyRegenLoopTimerScheduledIdUnique<'ctx> {
    /// Find the subscribed row whose `scheduled_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<TeleportationEnergyRegenLoopTimer> {
        self.imp.find(col_val)
    }
}
