// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::player_deployable_mount_request_type::PlayerDeployableMountRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct DeployableMountArgs {
    pub request: PlayerDeployableMountRequest,
}

impl From<DeployableMountArgs> for super::Reducer {
    fn from(args: DeployableMountArgs) -> Self {
        Self::DeployableMount {
            request: args.request,
        }
    }
}

impl __sdk::InModule for DeployableMountArgs {
    type Module = super::RemoteModule;
}

pub struct DeployableMountCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `deployable_mount`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait deployable_mount {
    /// Request that the remote module invoke the reducer `deployable_mount` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_deployable_mount`] callbacks.
    fn deployable_mount(&self, request: PlayerDeployableMountRequest) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `deployable_mount`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`DeployableMountCallbackId`] can be passed to [`Self::remove_on_deployable_mount`]
    /// to cancel the callback.
    fn on_deployable_mount(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &PlayerDeployableMountRequest)
        + Send
        + 'static,
    ) -> DeployableMountCallbackId;
    /// Cancel a callback previously registered by [`Self::on_deployable_mount`],
    /// causing it not to run in the future.
    fn remove_on_deployable_mount(&self, callback: DeployableMountCallbackId);
}

impl deployable_mount for super::RemoteReducers {
    fn deployable_mount(&self, request: PlayerDeployableMountRequest) -> __sdk::Result<()> {
        self.imp
            .call_reducer("deployable_mount", DeployableMountArgs { request })
    }
    fn on_deployable_mount(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &PlayerDeployableMountRequest)
        + Send
        + 'static,
    ) -> DeployableMountCallbackId {
        DeployableMountCallbackId(self.imp.on_reducer(
            "deployable_mount",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::DeployableMount { request },
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
    fn remove_on_deployable_mount(&self, callback: DeployableMountCallbackId) {
        self.imp.remove_on_reducer("deployable_mount", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `deployable_mount`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_deployable_mount {
    /// Set the call-reducer flags for the reducer `deployable_mount` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn deployable_mount(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_deployable_mount for super::SetReducerFlags {
    fn deployable_mount(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("deployable_mount", flags);
    }
}
