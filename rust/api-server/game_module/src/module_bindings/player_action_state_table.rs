// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::player_action_layer_type::PlayerActionLayer;
use super::player_action_result_type::PlayerActionResult;
use super::player_action_state_type::PlayerActionState;
use super::player_action_type_type::PlayerActionType;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `player_action_state`.
///
/// Obtain a handle from the [`PlayerActionStateTableAccess::player_action_state`] method on [`super::RemoteTables`],
/// like `ctx.db.player_action_state()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.player_action_state().on_insert(...)`.
pub struct PlayerActionStateTableHandle<'ctx> {
    imp: __sdk::TableHandle<PlayerActionState>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `player_action_state`.
///
/// Implemented for [`super::RemoteTables`].
pub trait PlayerActionStateTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`PlayerActionStateTableHandle`], which mediates access to the table `player_action_state`.
    fn player_action_state(&self) -> PlayerActionStateTableHandle<'_>;
}

impl PlayerActionStateTableAccess for super::RemoteTables {
    fn player_action_state(&self) -> PlayerActionStateTableHandle<'_> {
        PlayerActionStateTableHandle {
            imp: self
                .imp
                .get_table::<PlayerActionState>("player_action_state"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct PlayerActionStateInsertCallbackId(__sdk::CallbackId);
pub struct PlayerActionStateDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for PlayerActionStateTableHandle<'ctx> {
    type Row = PlayerActionState;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = PlayerActionState> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = PlayerActionStateInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> PlayerActionStateInsertCallbackId {
        PlayerActionStateInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: PlayerActionStateInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = PlayerActionStateDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> PlayerActionStateDeleteCallbackId {
        PlayerActionStateDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: PlayerActionStateDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<PlayerActionState>("player_action_state");
    _table.add_unique_constraint::<u64>("auto_id", |row| &row.auto_id);
}
pub struct PlayerActionStateUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for PlayerActionStateTableHandle<'ctx> {
    type UpdateCallbackId = PlayerActionStateUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> PlayerActionStateUpdateCallbackId {
        PlayerActionStateUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: PlayerActionStateUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<PlayerActionState>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<PlayerActionState>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `auto_id` unique index on the table `player_action_state`,
/// which allows point queries on the field of the same name
/// via the [`PlayerActionStateAutoIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.player_action_state().auto_id().find(...)`.
pub struct PlayerActionStateAutoIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<PlayerActionState, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> PlayerActionStateTableHandle<'ctx> {
    /// Get a handle on the `auto_id` unique index on the table `player_action_state`.
    pub fn auto_id(&self) -> PlayerActionStateAutoIdUnique<'ctx> {
        PlayerActionStateAutoIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("auto_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> PlayerActionStateAutoIdUnique<'ctx> {
    /// Find the subscribed row whose `auto_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<PlayerActionState> {
        self.imp.find(col_val)
    }
}
