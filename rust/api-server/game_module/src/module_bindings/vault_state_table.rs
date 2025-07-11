// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::vault_collectible_type::VaultCollectible;
use super::vault_state_type::VaultState;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `vault_state`.
///
/// Obtain a handle from the [`VaultStateTableAccess::vault_state`] method on [`super::RemoteTables`],
/// like `ctx.db.vault_state()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.vault_state().on_insert(...)`.
pub struct VaultStateTableHandle<'ctx> {
    imp: __sdk::TableHandle<VaultState>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `vault_state`.
///
/// Implemented for [`super::RemoteTables`].
pub trait VaultStateTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`VaultStateTableHandle`], which mediates access to the table `vault_state`.
    fn vault_state(&self) -> VaultStateTableHandle<'_>;
}

impl VaultStateTableAccess for super::RemoteTables {
    fn vault_state(&self) -> VaultStateTableHandle<'_> {
        VaultStateTableHandle {
            imp: self.imp.get_table::<VaultState>("vault_state"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct VaultStateInsertCallbackId(__sdk::CallbackId);
pub struct VaultStateDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for VaultStateTableHandle<'ctx> {
    type Row = VaultState;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = VaultState> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = VaultStateInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> VaultStateInsertCallbackId {
        VaultStateInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: VaultStateInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = VaultStateDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> VaultStateDeleteCallbackId {
        VaultStateDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: VaultStateDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<VaultState>("vault_state");
    _table.add_unique_constraint::<u64>("entity_id", |row| &row.entity_id);
}
pub struct VaultStateUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for VaultStateTableHandle<'ctx> {
    type UpdateCallbackId = VaultStateUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> VaultStateUpdateCallbackId {
        VaultStateUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: VaultStateUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<VaultState>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<VaultState>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `entity_id` unique index on the table `vault_state`,
/// which allows point queries on the field of the same name
/// via the [`VaultStateEntityIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.vault_state().entity_id().find(...)`.
pub struct VaultStateEntityIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<VaultState, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> VaultStateTableHandle<'ctx> {
    /// Get a handle on the `entity_id` unique index on the table `vault_state`.
    pub fn entity_id(&self) -> VaultStateEntityIdUnique<'ctx> {
        VaultStateEntityIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("entity_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> VaultStateEntityIdUnique<'ctx> {
    /// Find the subscribed row whose `entity_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<VaultState> {
        self.imp.find(col_val)
    }
}
