// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::player_housing_request_access_request_type::PlayerHousingRequestAccessRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct PlayerHousingRequestAccessArgs {
    pub request: PlayerHousingRequestAccessRequest,
}

impl From<PlayerHousingRequestAccessArgs> for super::Reducer {
    fn from(args: PlayerHousingRequestAccessArgs) -> Self {
        Self::PlayerHousingRequestAccess {
            request: args.request,
        }
    }
}

impl __sdk::InModule for PlayerHousingRequestAccessArgs {
    type Module = super::RemoteModule;
}

pub struct PlayerHousingRequestAccessCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `player_housing_request_access`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait player_housing_request_access {
    /// Request that the remote module invoke the reducer `player_housing_request_access` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_player_housing_request_access`] callbacks.
    fn player_housing_request_access(
        &self,
        request: PlayerHousingRequestAccessRequest,
    ) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `player_housing_request_access`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`PlayerHousingRequestAccessCallbackId`] can be passed to [`Self::remove_on_player_housing_request_access`]
    /// to cancel the callback.
    fn on_player_housing_request_access(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &PlayerHousingRequestAccessRequest)
        + Send
        + 'static,
    ) -> PlayerHousingRequestAccessCallbackId;
    /// Cancel a callback previously registered by [`Self::on_player_housing_request_access`],
    /// causing it not to run in the future.
    fn remove_on_player_housing_request_access(
        &self,
        callback: PlayerHousingRequestAccessCallbackId,
    );
}

impl player_housing_request_access for super::RemoteReducers {
    fn player_housing_request_access(
        &self,
        request: PlayerHousingRequestAccessRequest,
    ) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "player_housing_request_access",
            PlayerHousingRequestAccessArgs { request },
        )
    }
    fn on_player_housing_request_access(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &PlayerHousingRequestAccessRequest)
        + Send
        + 'static,
    ) -> PlayerHousingRequestAccessCallbackId {
        PlayerHousingRequestAccessCallbackId(self.imp.on_reducer(
            "player_housing_request_access",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::PlayerHousingRequestAccess { request },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx, request)
            }),
        ))
    }
    fn remove_on_player_housing_request_access(
        &self,
        callback: PlayerHousingRequestAccessCallbackId,
    ) {
        self.imp
            .remove_on_reducer("player_housing_request_access", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `player_housing_request_access`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_player_housing_request_access {
    /// Set the call-reducer flags for the reducer `player_housing_request_access` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn player_housing_request_access(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_player_housing_request_access for super::SetReducerFlags {
    fn player_housing_request_access(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("player_housing_request_access", flags);
    }
}
