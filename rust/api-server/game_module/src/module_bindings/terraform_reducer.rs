// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::player_terraform_request_type::PlayerTerraformRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct TerraformArgs {
    pub request: PlayerTerraformRequest,
}

impl From<TerraformArgs> for super::Reducer {
    fn from(args: TerraformArgs) -> Self {
        Self::Terraform {
            request: args.request,
        }
    }
}

impl __sdk::InModule for TerraformArgs {
    type Module = super::RemoteModule;
}

pub struct TerraformCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `terraform`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait terraform {
    /// Request that the remote module invoke the reducer `terraform` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_terraform`] callbacks.
    fn terraform(&self, request: PlayerTerraformRequest) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `terraform`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`TerraformCallbackId`] can be passed to [`Self::remove_on_terraform`]
    /// to cancel the callback.
    fn on_terraform(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &PlayerTerraformRequest) + Send + 'static,
    ) -> TerraformCallbackId;
    /// Cancel a callback previously registered by [`Self::on_terraform`],
    /// causing it not to run in the future.
    fn remove_on_terraform(&self, callback: TerraformCallbackId);
}

impl terraform for super::RemoteReducers {
    fn terraform(&self, request: PlayerTerraformRequest) -> __sdk::Result<()> {
        self.imp
            .call_reducer("terraform", TerraformArgs { request })
    }
    fn on_terraform(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &PlayerTerraformRequest) + Send + 'static,
    ) -> TerraformCallbackId {
        TerraformCallbackId(self.imp.on_reducer(
            "terraform",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::Terraform { request },
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
    fn remove_on_terraform(&self, callback: TerraformCallbackId) {
        self.imp.remove_on_reducer("terraform", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `terraform`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_terraform {
    /// Set the call-reducer flags for the reducer `terraform` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn terraform(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_terraform for super::SetReducerFlags {
    fn terraform(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("terraform", flags);
    }
}
