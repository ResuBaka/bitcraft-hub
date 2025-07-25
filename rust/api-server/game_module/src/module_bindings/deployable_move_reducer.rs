// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::player_deployable_move_request_type::PlayerDeployableMoveRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct DeployableMoveArgs {
    pub request: PlayerDeployableMoveRequest,
}

impl From<DeployableMoveArgs> for super::Reducer {
    fn from(args: DeployableMoveArgs) -> Self {
        Self::DeployableMove {
            request: args.request,
        }
    }
}

impl __sdk::InModule for DeployableMoveArgs {
    type Module = super::RemoteModule;
}

pub struct DeployableMoveCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `deployable_move`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait deployable_move {
    /// Request that the remote module invoke the reducer `deployable_move` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_deployable_move`] callbacks.
    fn deployable_move(&self, request: PlayerDeployableMoveRequest) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `deployable_move`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`DeployableMoveCallbackId`] can be passed to [`Self::remove_on_deployable_move`]
    /// to cancel the callback.
    fn on_deployable_move(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &PlayerDeployableMoveRequest) + Send + 'static,
    ) -> DeployableMoveCallbackId;
    /// Cancel a callback previously registered by [`Self::on_deployable_move`],
    /// causing it not to run in the future.
    fn remove_on_deployable_move(&self, callback: DeployableMoveCallbackId);
}

impl deployable_move for super::RemoteReducers {
    fn deployable_move(&self, request: PlayerDeployableMoveRequest) -> __sdk::Result<()> {
        self.imp
            .call_reducer("deployable_move", DeployableMoveArgs { request })
    }
    fn on_deployable_move(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &PlayerDeployableMoveRequest)
        + Send
        + 'static,
    ) -> DeployableMoveCallbackId {
        DeployableMoveCallbackId(self.imp.on_reducer(
            "deployable_move",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::DeployableMove { request },
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
    fn remove_on_deployable_move(&self, callback: DeployableMoveCallbackId) {
        self.imp.remove_on_reducer("deployable_move", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `deployable_move`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_deployable_move {
    /// Set the call-reducer flags for the reducer `deployable_move` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn deployable_move(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_deployable_move for super::SetReducerFlags {
    fn deployable_move(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("deployable_move", flags);
    }
}
