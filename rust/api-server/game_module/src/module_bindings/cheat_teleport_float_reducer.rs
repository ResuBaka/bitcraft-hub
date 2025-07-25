// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::cheat_teleport_float_request_type::CheatTeleportFloatRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct CheatTeleportFloatArgs {
    pub request: CheatTeleportFloatRequest,
}

impl From<CheatTeleportFloatArgs> for super::Reducer {
    fn from(args: CheatTeleportFloatArgs) -> Self {
        Self::CheatTeleportFloat {
            request: args.request,
        }
    }
}

impl __sdk::InModule for CheatTeleportFloatArgs {
    type Module = super::RemoteModule;
}

pub struct CheatTeleportFloatCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `cheat_teleport_float`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait cheat_teleport_float {
    /// Request that the remote module invoke the reducer `cheat_teleport_float` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_cheat_teleport_float`] callbacks.
    fn cheat_teleport_float(&self, request: CheatTeleportFloatRequest) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `cheat_teleport_float`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`CheatTeleportFloatCallbackId`] can be passed to [`Self::remove_on_cheat_teleport_float`]
    /// to cancel the callback.
    fn on_cheat_teleport_float(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &CheatTeleportFloatRequest) + Send + 'static,
    ) -> CheatTeleportFloatCallbackId;
    /// Cancel a callback previously registered by [`Self::on_cheat_teleport_float`],
    /// causing it not to run in the future.
    fn remove_on_cheat_teleport_float(&self, callback: CheatTeleportFloatCallbackId);
}

impl cheat_teleport_float for super::RemoteReducers {
    fn cheat_teleport_float(&self, request: CheatTeleportFloatRequest) -> __sdk::Result<()> {
        self.imp
            .call_reducer("cheat_teleport_float", CheatTeleportFloatArgs { request })
    }
    fn on_cheat_teleport_float(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &CheatTeleportFloatRequest)
        + Send
        + 'static,
    ) -> CheatTeleportFloatCallbackId {
        CheatTeleportFloatCallbackId(self.imp.on_reducer(
            "cheat_teleport_float",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::CheatTeleportFloat { request },
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
    fn remove_on_cheat_teleport_float(&self, callback: CheatTeleportFloatCallbackId) {
        self.imp
            .remove_on_reducer("cheat_teleport_float", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `cheat_teleport_float`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_cheat_teleport_float {
    /// Set the call-reducer flags for the reducer `cheat_teleport_float` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn cheat_teleport_float(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_cheat_teleport_float for super::SetReducerFlags {
    fn cheat_teleport_float(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("cheat_teleport_float", flags);
    }
}
