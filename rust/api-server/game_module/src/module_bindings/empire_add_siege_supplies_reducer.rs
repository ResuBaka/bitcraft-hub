// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::empire_add_siege_supplies_request_type::EmpireAddSiegeSuppliesRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct EmpireAddSiegeSuppliesArgs {
    pub request: EmpireAddSiegeSuppliesRequest,
}

impl From<EmpireAddSiegeSuppliesArgs> for super::Reducer {
    fn from(args: EmpireAddSiegeSuppliesArgs) -> Self {
        Self::EmpireAddSiegeSupplies {
            request: args.request,
        }
    }
}

impl __sdk::InModule for EmpireAddSiegeSuppliesArgs {
    type Module = super::RemoteModule;
}

pub struct EmpireAddSiegeSuppliesCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `empire_add_siege_supplies`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait empire_add_siege_supplies {
    /// Request that the remote module invoke the reducer `empire_add_siege_supplies` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_empire_add_siege_supplies`] callbacks.
    fn empire_add_siege_supplies(
        &self,
        request: EmpireAddSiegeSuppliesRequest,
    ) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `empire_add_siege_supplies`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`EmpireAddSiegeSuppliesCallbackId`] can be passed to [`Self::remove_on_empire_add_siege_supplies`]
    /// to cancel the callback.
    fn on_empire_add_siege_supplies(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &EmpireAddSiegeSuppliesRequest)
        + Send
        + 'static,
    ) -> EmpireAddSiegeSuppliesCallbackId;
    /// Cancel a callback previously registered by [`Self::on_empire_add_siege_supplies`],
    /// causing it not to run in the future.
    fn remove_on_empire_add_siege_supplies(&self, callback: EmpireAddSiegeSuppliesCallbackId);
}

impl empire_add_siege_supplies for super::RemoteReducers {
    fn empire_add_siege_supplies(
        &self,
        request: EmpireAddSiegeSuppliesRequest,
    ) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "empire_add_siege_supplies",
            EmpireAddSiegeSuppliesArgs { request },
        )
    }
    fn on_empire_add_siege_supplies(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &EmpireAddSiegeSuppliesRequest)
        + Send
        + 'static,
    ) -> EmpireAddSiegeSuppliesCallbackId {
        EmpireAddSiegeSuppliesCallbackId(self.imp.on_reducer(
            "empire_add_siege_supplies",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::EmpireAddSiegeSupplies { request },
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
    fn remove_on_empire_add_siege_supplies(&self, callback: EmpireAddSiegeSuppliesCallbackId) {
        self.imp
            .remove_on_reducer("empire_add_siege_supplies", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `empire_add_siege_supplies`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_empire_add_siege_supplies {
    /// Set the call-reducer flags for the reducer `empire_add_siege_supplies` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn empire_add_siege_supplies(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_empire_add_siege_supplies for super::SetReducerFlags {
    fn empire_add_siege_supplies(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("empire_add_siege_supplies", flags);
    }
}
