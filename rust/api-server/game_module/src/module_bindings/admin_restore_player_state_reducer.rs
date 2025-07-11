// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct AdminRestorePlayerStateArgs {
    pub username: String,
    pub restore_position: bool,
    pub force_signout: bool,
    pub restore_all_deployables_positions: bool,
    pub store_deployables: bool,
    pub clear_cargo: bool,
    pub clear_items: bool,
    pub clear_toolbelt: bool,
}

impl From<AdminRestorePlayerStateArgs> for super::Reducer {
    fn from(args: AdminRestorePlayerStateArgs) -> Self {
        Self::AdminRestorePlayerState {
            username: args.username,
            restore_position: args.restore_position,
            force_signout: args.force_signout,
            restore_all_deployables_positions: args.restore_all_deployables_positions,
            store_deployables: args.store_deployables,
            clear_cargo: args.clear_cargo,
            clear_items: args.clear_items,
            clear_toolbelt: args.clear_toolbelt,
        }
    }
}

impl __sdk::InModule for AdminRestorePlayerStateArgs {
    type Module = super::RemoteModule;
}

pub struct AdminRestorePlayerStateCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `admin_restore_player_state`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait admin_restore_player_state {
    /// Request that the remote module invoke the reducer `admin_restore_player_state` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_admin_restore_player_state`] callbacks.
    fn admin_restore_player_state(
        &self,
        username: String,
        restore_position: bool,
        force_signout: bool,
        restore_all_deployables_positions: bool,
        store_deployables: bool,
        clear_cargo: bool,
        clear_items: bool,
        clear_toolbelt: bool,
    ) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `admin_restore_player_state`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`AdminRestorePlayerStateCallbackId`] can be passed to [`Self::remove_on_admin_restore_player_state`]
    /// to cancel the callback.
    fn on_admin_restore_player_state(
        &self,
        callback: impl FnMut(
            &super::ReducerEventContext,
            &String,
            &bool,
            &bool,
            &bool,
            &bool,
            &bool,
            &bool,
            &bool,
        ) + Send
        + 'static,
    ) -> AdminRestorePlayerStateCallbackId;
    /// Cancel a callback previously registered by [`Self::on_admin_restore_player_state`],
    /// causing it not to run in the future.
    fn remove_on_admin_restore_player_state(&self, callback: AdminRestorePlayerStateCallbackId);
}

impl admin_restore_player_state for super::RemoteReducers {
    fn admin_restore_player_state(
        &self,
        username: String,
        restore_position: bool,
        force_signout: bool,
        restore_all_deployables_positions: bool,
        store_deployables: bool,
        clear_cargo: bool,
        clear_items: bool,
        clear_toolbelt: bool,
    ) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "admin_restore_player_state",
            AdminRestorePlayerStateArgs {
                username,
                restore_position,
                force_signout,
                restore_all_deployables_positions,
                store_deployables,
                clear_cargo,
                clear_items,
                clear_toolbelt,
            },
        )
    }
    fn on_admin_restore_player_state(
        &self,
        mut callback: impl FnMut(
            &super::ReducerEventContext,
            &String,
            &bool,
            &bool,
            &bool,
            &bool,
            &bool,
            &bool,
            &bool,
        ) + Send
        + 'static,
    ) -> AdminRestorePlayerStateCallbackId {
        AdminRestorePlayerStateCallbackId(self.imp.on_reducer(
            "admin_restore_player_state",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer:
                                super::Reducer::AdminRestorePlayerState {
                                    username,
                                    restore_position,
                                    force_signout,
                                    restore_all_deployables_positions,
                                    store_deployables,
                                    clear_cargo,
                                    clear_items,
                                    clear_toolbelt,
                                },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(
                    ctx,
                    username,
                    restore_position,
                    force_signout,
                    restore_all_deployables_positions,
                    store_deployables,
                    clear_cargo,
                    clear_items,
                    clear_toolbelt,
                )
            }),
        ))
    }
    fn remove_on_admin_restore_player_state(&self, callback: AdminRestorePlayerStateCallbackId) {
        self.imp
            .remove_on_reducer("admin_restore_player_state", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `admin_restore_player_state`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_admin_restore_player_state {
    /// Set the call-reducer flags for the reducer `admin_restore_player_state` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn admin_restore_player_state(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_admin_restore_player_state for super::SetReducerFlags {
    fn admin_restore_player_state(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("admin_restore_player_state", flags);
    }
}
