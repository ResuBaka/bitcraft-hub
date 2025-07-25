// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::player_collectible_activate_request_type::PlayerCollectibleActivateRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct CollectibleActivateArgs {
    pub request: PlayerCollectibleActivateRequest,
}

impl From<CollectibleActivateArgs> for super::Reducer {
    fn from(args: CollectibleActivateArgs) -> Self {
        Self::CollectibleActivate {
            request: args.request,
        }
    }
}

impl __sdk::InModule for CollectibleActivateArgs {
    type Module = super::RemoteModule;
}

pub struct CollectibleActivateCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `collectible_activate`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait collectible_activate {
    /// Request that the remote module invoke the reducer `collectible_activate` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_collectible_activate`] callbacks.
    fn collectible_activate(&self, request: PlayerCollectibleActivateRequest) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `collectible_activate`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`CollectibleActivateCallbackId`] can be passed to [`Self::remove_on_collectible_activate`]
    /// to cancel the callback.
    fn on_collectible_activate(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &PlayerCollectibleActivateRequest)
        + Send
        + 'static,
    ) -> CollectibleActivateCallbackId;
    /// Cancel a callback previously registered by [`Self::on_collectible_activate`],
    /// causing it not to run in the future.
    fn remove_on_collectible_activate(&self, callback: CollectibleActivateCallbackId);
}

impl collectible_activate for super::RemoteReducers {
    fn collectible_activate(&self, request: PlayerCollectibleActivateRequest) -> __sdk::Result<()> {
        self.imp
            .call_reducer("collectible_activate", CollectibleActivateArgs { request })
    }
    fn on_collectible_activate(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &PlayerCollectibleActivateRequest)
        + Send
        + 'static,
    ) -> CollectibleActivateCallbackId {
        CollectibleActivateCallbackId(self.imp.on_reducer(
            "collectible_activate",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::CollectibleActivate { request },
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
    fn remove_on_collectible_activate(&self, callback: CollectibleActivateCallbackId) {
        self.imp
            .remove_on_reducer("collectible_activate", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `collectible_activate`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_collectible_activate {
    /// Set the call-reducer flags for the reducer `collectible_activate` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn collectible_activate(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_collectible_activate for super::SetReducerFlags {
    fn collectible_activate(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("collectible_activate", flags);
    }
}
