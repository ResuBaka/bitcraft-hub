// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::rent_terminate_request_type::RentTerminateRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct RentTerminateArgs {
    pub request: RentTerminateRequest,
}

impl From<RentTerminateArgs> for super::Reducer {
    fn from(args: RentTerminateArgs) -> Self {
        Self::RentTerminate {
            request: args.request,
        }
    }
}

impl __sdk::InModule for RentTerminateArgs {
    type Module = super::RemoteModule;
}

pub struct RentTerminateCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `rent_terminate`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait rent_terminate {
    /// Request that the remote module invoke the reducer `rent_terminate` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_rent_terminate`] callbacks.
    fn rent_terminate(&self, request: RentTerminateRequest) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `rent_terminate`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`RentTerminateCallbackId`] can be passed to [`Self::remove_on_rent_terminate`]
    /// to cancel the callback.
    fn on_rent_terminate(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &RentTerminateRequest) + Send + 'static,
    ) -> RentTerminateCallbackId;
    /// Cancel a callback previously registered by [`Self::on_rent_terminate`],
    /// causing it not to run in the future.
    fn remove_on_rent_terminate(&self, callback: RentTerminateCallbackId);
}

impl rent_terminate for super::RemoteReducers {
    fn rent_terminate(&self, request: RentTerminateRequest) -> __sdk::Result<()> {
        self.imp
            .call_reducer("rent_terminate", RentTerminateArgs { request })
    }
    fn on_rent_terminate(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &RentTerminateRequest) + Send + 'static,
    ) -> RentTerminateCallbackId {
        RentTerminateCallbackId(self.imp.on_reducer(
            "rent_terminate",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::RentTerminate { request },
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
    fn remove_on_rent_terminate(&self, callback: RentTerminateCallbackId) {
        self.imp.remove_on_reducer("rent_terminate", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `rent_terminate`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_rent_terminate {
    /// Set the call-reducer flags for the reducer `rent_terminate` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn rent_terminate(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_rent_terminate for super::SetReducerFlags {
    fn rent_terminate(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("rent_terminate", flags);
    }
}
