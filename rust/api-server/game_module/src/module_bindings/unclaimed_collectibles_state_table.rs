// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::unclaimed_collectibles_state_type::UnclaimedCollectiblesState;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `unclaimed_collectibles_state`.
///
/// Obtain a handle from the [`UnclaimedCollectiblesStateTableAccess::unclaimed_collectibles_state`] method on [`super::RemoteTables`],
/// like `ctx.db.unclaimed_collectibles_state()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.unclaimed_collectibles_state().on_insert(...)`.
pub struct UnclaimedCollectiblesStateTableHandle<'ctx> {
    imp: __sdk::TableHandle<UnclaimedCollectiblesState>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `unclaimed_collectibles_state`.
///
/// Implemented for [`super::RemoteTables`].
pub trait UnclaimedCollectiblesStateTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`UnclaimedCollectiblesStateTableHandle`], which mediates access to the table `unclaimed_collectibles_state`.
    fn unclaimed_collectibles_state(&self) -> UnclaimedCollectiblesStateTableHandle<'_>;
}

impl UnclaimedCollectiblesStateTableAccess for super::RemoteTables {
    fn unclaimed_collectibles_state(&self) -> UnclaimedCollectiblesStateTableHandle<'_> {
        UnclaimedCollectiblesStateTableHandle {
            imp: self
                .imp
                .get_table::<UnclaimedCollectiblesState>("unclaimed_collectibles_state"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct UnclaimedCollectiblesStateInsertCallbackId(__sdk::CallbackId);
pub struct UnclaimedCollectiblesStateDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for UnclaimedCollectiblesStateTableHandle<'ctx> {
    type Row = UnclaimedCollectiblesState;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = UnclaimedCollectiblesState> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = UnclaimedCollectiblesStateInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> UnclaimedCollectiblesStateInsertCallbackId {
        UnclaimedCollectiblesStateInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: UnclaimedCollectiblesStateInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = UnclaimedCollectiblesStateDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> UnclaimedCollectiblesStateDeleteCallbackId {
        UnclaimedCollectiblesStateDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: UnclaimedCollectiblesStateDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache
        .get_or_make_table::<UnclaimedCollectiblesState>("unclaimed_collectibles_state");
    _table.add_unique_constraint::<__sdk::Identity>("identity", |row| &row.identity);
}
pub struct UnclaimedCollectiblesStateUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for UnclaimedCollectiblesStateTableHandle<'ctx> {
    type UpdateCallbackId = UnclaimedCollectiblesStateUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> UnclaimedCollectiblesStateUpdateCallbackId {
        UnclaimedCollectiblesStateUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: UnclaimedCollectiblesStateUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<UnclaimedCollectiblesState>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<UnclaimedCollectiblesState>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `identity` unique index on the table `unclaimed_collectibles_state`,
/// which allows point queries on the field of the same name
/// via the [`UnclaimedCollectiblesStateIdentityUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.unclaimed_collectibles_state().identity().find(...)`.
pub struct UnclaimedCollectiblesStateIdentityUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<UnclaimedCollectiblesState, __sdk::Identity>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> UnclaimedCollectiblesStateTableHandle<'ctx> {
    /// Get a handle on the `identity` unique index on the table `unclaimed_collectibles_state`.
    pub fn identity(&self) -> UnclaimedCollectiblesStateIdentityUnique<'ctx> {
        UnclaimedCollectiblesStateIdentityUnique {
            imp: self
                .imp
                .get_unique_constraint::<__sdk::Identity>("identity"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> UnclaimedCollectiblesStateIdentityUnique<'ctx> {
    /// Find the subscribed row whose `identity` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &__sdk::Identity) -> Option<UnclaimedCollectiblesState> {
        self.imp.find(col_val)
    }
}
