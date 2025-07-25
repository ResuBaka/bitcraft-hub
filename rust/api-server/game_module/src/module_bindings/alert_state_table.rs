// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::alert_state_type::AlertState;
use super::alert_type_type::AlertType;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `alert_state`.
///
/// Obtain a handle from the [`AlertStateTableAccess::alert_state`] method on [`super::RemoteTables`],
/// like `ctx.db.alert_state()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.alert_state().on_insert(...)`.
pub struct AlertStateTableHandle<'ctx> {
    imp: __sdk::TableHandle<AlertState>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `alert_state`.
///
/// Implemented for [`super::RemoteTables`].
pub trait AlertStateTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`AlertStateTableHandle`], which mediates access to the table `alert_state`.
    fn alert_state(&self) -> AlertStateTableHandle<'_>;
}

impl AlertStateTableAccess for super::RemoteTables {
    fn alert_state(&self) -> AlertStateTableHandle<'_> {
        AlertStateTableHandle {
            imp: self.imp.get_table::<AlertState>("alert_state"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct AlertStateInsertCallbackId(__sdk::CallbackId);
pub struct AlertStateDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for AlertStateTableHandle<'ctx> {
    type Row = AlertState;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = AlertState> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = AlertStateInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> AlertStateInsertCallbackId {
        AlertStateInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: AlertStateInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = AlertStateDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> AlertStateDeleteCallbackId {
        AlertStateDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: AlertStateDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<AlertState>("alert_state");
    _table.add_unique_constraint::<u64>("entity_id", |row| &row.entity_id);
}
pub struct AlertStateUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for AlertStateTableHandle<'ctx> {
    type UpdateCallbackId = AlertStateUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> AlertStateUpdateCallbackId {
        AlertStateUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: AlertStateUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<AlertState>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<AlertState>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `entity_id` unique index on the table `alert_state`,
/// which allows point queries on the field of the same name
/// via the [`AlertStateEntityIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.alert_state().entity_id().find(...)`.
pub struct AlertStateEntityIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<AlertState, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> AlertStateTableHandle<'ctx> {
    /// Get a handle on the `entity_id` unique index on the table `alert_state`.
    pub fn entity_id(&self) -> AlertStateEntityIdUnique<'ctx> {
        AlertStateEntityIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("entity_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> AlertStateEntityIdUnique<'ctx> {
    /// Find the subscribed row whose `entity_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<AlertState> {
        self.imp.find(col_val)
    }
}
