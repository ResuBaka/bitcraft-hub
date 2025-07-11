// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::player_claim_withdraw_from_treasury_request_type::PlayerClaimWithdrawFromTreasuryRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct ClaimWithdrawFromTreasuryArgs {
    pub request: PlayerClaimWithdrawFromTreasuryRequest,
}

impl From<ClaimWithdrawFromTreasuryArgs> for super::Reducer {
    fn from(args: ClaimWithdrawFromTreasuryArgs) -> Self {
        Self::ClaimWithdrawFromTreasury {
            request: args.request,
        }
    }
}

impl __sdk::InModule for ClaimWithdrawFromTreasuryArgs {
    type Module = super::RemoteModule;
}

pub struct ClaimWithdrawFromTreasuryCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `claim_withdraw_from_treasury`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait claim_withdraw_from_treasury {
    /// Request that the remote module invoke the reducer `claim_withdraw_from_treasury` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_claim_withdraw_from_treasury`] callbacks.
    fn claim_withdraw_from_treasury(
        &self,
        request: PlayerClaimWithdrawFromTreasuryRequest,
    ) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `claim_withdraw_from_treasury`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`ClaimWithdrawFromTreasuryCallbackId`] can be passed to [`Self::remove_on_claim_withdraw_from_treasury`]
    /// to cancel the callback.
    fn on_claim_withdraw_from_treasury(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &PlayerClaimWithdrawFromTreasuryRequest)
        + Send
        + 'static,
    ) -> ClaimWithdrawFromTreasuryCallbackId;
    /// Cancel a callback previously registered by [`Self::on_claim_withdraw_from_treasury`],
    /// causing it not to run in the future.
    fn remove_on_claim_withdraw_from_treasury(&self, callback: ClaimWithdrawFromTreasuryCallbackId);
}

impl claim_withdraw_from_treasury for super::RemoteReducers {
    fn claim_withdraw_from_treasury(
        &self,
        request: PlayerClaimWithdrawFromTreasuryRequest,
    ) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "claim_withdraw_from_treasury",
            ClaimWithdrawFromTreasuryArgs { request },
        )
    }
    fn on_claim_withdraw_from_treasury(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &PlayerClaimWithdrawFromTreasuryRequest)
        + Send
        + 'static,
    ) -> ClaimWithdrawFromTreasuryCallbackId {
        ClaimWithdrawFromTreasuryCallbackId(self.imp.on_reducer(
            "claim_withdraw_from_treasury",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::ClaimWithdrawFromTreasury { request },
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
    fn remove_on_claim_withdraw_from_treasury(
        &self,
        callback: ClaimWithdrawFromTreasuryCallbackId,
    ) {
        self.imp
            .remove_on_reducer("claim_withdraw_from_treasury", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `claim_withdraw_from_treasury`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_claim_withdraw_from_treasury {
    /// Set the call-reducer flags for the reducer `claim_withdraw_from_treasury` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn claim_withdraw_from_treasury(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_claim_withdraw_from_treasury for super::SetReducerFlags {
    fn claim_withdraw_from_treasury(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("claim_withdraw_from_treasury", flags);
    }
}
