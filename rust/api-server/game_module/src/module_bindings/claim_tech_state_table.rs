// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::claim_tech_state_type::ClaimTechState;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `claim_tech_state`.
///
/// Obtain a handle from the [`ClaimTechStateTableAccess::claim_tech_state`] method on [`super::RemoteTables`],
/// like `ctx.db.claim_tech_state()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.claim_tech_state().on_insert(...)`.
pub struct ClaimTechStateTableHandle<'ctx> {
    imp: __sdk::TableHandle<ClaimTechState>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `claim_tech_state`.
///
/// Implemented for [`super::RemoteTables`].
pub trait ClaimTechStateTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`ClaimTechStateTableHandle`], which mediates access to the table `claim_tech_state`.
    fn claim_tech_state(&self) -> ClaimTechStateTableHandle<'_>;
}

impl ClaimTechStateTableAccess for super::RemoteTables {
    fn claim_tech_state(&self) -> ClaimTechStateTableHandle<'_> {
        ClaimTechStateTableHandle {
            imp: self.imp.get_table::<ClaimTechState>("claim_tech_state"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct ClaimTechStateInsertCallbackId(__sdk::CallbackId);
pub struct ClaimTechStateDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for ClaimTechStateTableHandle<'ctx> {
    type Row = ClaimTechState;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = ClaimTechState> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = ClaimTechStateInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> ClaimTechStateInsertCallbackId {
        ClaimTechStateInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: ClaimTechStateInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = ClaimTechStateDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> ClaimTechStateDeleteCallbackId {
        ClaimTechStateDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: ClaimTechStateDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<ClaimTechState>("claim_tech_state");
    _table.add_unique_constraint::<u64>("entity_id", |row| &row.entity_id);
}
pub struct ClaimTechStateUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for ClaimTechStateTableHandle<'ctx> {
    type UpdateCallbackId = ClaimTechStateUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> ClaimTechStateUpdateCallbackId {
        ClaimTechStateUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: ClaimTechStateUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<ClaimTechState>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<ClaimTechState>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `entity_id` unique index on the table `claim_tech_state`,
/// which allows point queries on the field of the same name
/// via the [`ClaimTechStateEntityIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.claim_tech_state().entity_id().find(...)`.
pub struct ClaimTechStateEntityIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<ClaimTechState, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> ClaimTechStateTableHandle<'ctx> {
    /// Get a handle on the `entity_id` unique index on the table `claim_tech_state`.
    pub fn entity_id(&self) -> ClaimTechStateEntityIdUnique<'ctx> {
        ClaimTechStateEntityIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("entity_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> ClaimTechStateEntityIdUnique<'ctx> {
    /// Find the subscribed row whose `entity_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<ClaimTechState> {
        self.imp.find(col_val)
    }
}
