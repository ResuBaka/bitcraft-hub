// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::player_username_state_type::PlayerUsernameState;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `player_username_state`.
///
/// Obtain a handle from the [`PlayerUsernameStateTableAccess::player_username_state`] method on [`super::RemoteTables`],
/// like `ctx.db.player_username_state()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.player_username_state().on_insert(...)`.
pub struct PlayerUsernameStateTableHandle<'ctx> {
    imp: __sdk::TableHandle<PlayerUsernameState>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `player_username_state`.
///
/// Implemented for [`super::RemoteTables`].
pub trait PlayerUsernameStateTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`PlayerUsernameStateTableHandle`], which mediates access to the table `player_username_state`.
    fn player_username_state(&self) -> PlayerUsernameStateTableHandle<'_>;
}

impl PlayerUsernameStateTableAccess for super::RemoteTables {
    fn player_username_state(&self) -> PlayerUsernameStateTableHandle<'_> {
        PlayerUsernameStateTableHandle {
            imp: self
                .imp
                .get_table::<PlayerUsernameState>("player_username_state"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct PlayerUsernameStateInsertCallbackId(__sdk::CallbackId);
pub struct PlayerUsernameStateDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for PlayerUsernameStateTableHandle<'ctx> {
    type Row = PlayerUsernameState;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = PlayerUsernameState> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = PlayerUsernameStateInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> PlayerUsernameStateInsertCallbackId {
        PlayerUsernameStateInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: PlayerUsernameStateInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = PlayerUsernameStateDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> PlayerUsernameStateDeleteCallbackId {
        PlayerUsernameStateDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: PlayerUsernameStateDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<PlayerUsernameState>("player_username_state");
    _table.add_unique_constraint::<u64>("entity_id", |row| &row.entity_id);
    _table.add_unique_constraint::<String>("username", |row| &row.username);
}
pub struct PlayerUsernameStateUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for PlayerUsernameStateTableHandle<'ctx> {
    type UpdateCallbackId = PlayerUsernameStateUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> PlayerUsernameStateUpdateCallbackId {
        PlayerUsernameStateUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: PlayerUsernameStateUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<PlayerUsernameState>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<PlayerUsernameState>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `entity_id` unique index on the table `player_username_state`,
/// which allows point queries on the field of the same name
/// via the [`PlayerUsernameStateEntityIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.player_username_state().entity_id().find(...)`.
pub struct PlayerUsernameStateEntityIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<PlayerUsernameState, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> PlayerUsernameStateTableHandle<'ctx> {
    /// Get a handle on the `entity_id` unique index on the table `player_username_state`.
    pub fn entity_id(&self) -> PlayerUsernameStateEntityIdUnique<'ctx> {
        PlayerUsernameStateEntityIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("entity_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> PlayerUsernameStateEntityIdUnique<'ctx> {
    /// Find the subscribed row whose `entity_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<PlayerUsernameState> {
        self.imp.find(col_val)
    }
}

/// Access to the `username` unique index on the table `player_username_state`,
/// which allows point queries on the field of the same name
/// via the [`PlayerUsernameStateUsernameUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.player_username_state().username().find(...)`.
pub struct PlayerUsernameStateUsernameUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<PlayerUsernameState, String>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> PlayerUsernameStateTableHandle<'ctx> {
    /// Get a handle on the `username` unique index on the table `player_username_state`.
    pub fn username(&self) -> PlayerUsernameStateUsernameUnique<'ctx> {
        PlayerUsernameStateUsernameUnique {
            imp: self.imp.get_unique_constraint::<String>("username"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> PlayerUsernameStateUsernameUnique<'ctx> {
    /// Find the subscribed row whose `username` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &String) -> Option<PlayerUsernameState> {
        self.imp.find(col_val)
    }
}
