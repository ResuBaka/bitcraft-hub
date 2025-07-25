// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct LogClaimMemberLeaderboardArgs {}

impl From<LogClaimMemberLeaderboardArgs> for super::Reducer {
    fn from(args: LogClaimMemberLeaderboardArgs) -> Self {
        Self::LogClaimMemberLeaderboard
    }
}

impl __sdk::InModule for LogClaimMemberLeaderboardArgs {
    type Module = super::RemoteModule;
}

pub struct LogClaimMemberLeaderboardCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `log_claim_member_leaderboard`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait log_claim_member_leaderboard {
    /// Request that the remote module invoke the reducer `log_claim_member_leaderboard` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_log_claim_member_leaderboard`] callbacks.
    fn log_claim_member_leaderboard(&self) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `log_claim_member_leaderboard`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`LogClaimMemberLeaderboardCallbackId`] can be passed to [`Self::remove_on_log_claim_member_leaderboard`]
    /// to cancel the callback.
    fn on_log_claim_member_leaderboard(
        &self,
        callback: impl FnMut(&super::ReducerEventContext) + Send + 'static,
    ) -> LogClaimMemberLeaderboardCallbackId;
    /// Cancel a callback previously registered by [`Self::on_log_claim_member_leaderboard`],
    /// causing it not to run in the future.
    fn remove_on_log_claim_member_leaderboard(&self, callback: LogClaimMemberLeaderboardCallbackId);
}

impl log_claim_member_leaderboard for super::RemoteReducers {
    fn log_claim_member_leaderboard(&self) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "log_claim_member_leaderboard",
            LogClaimMemberLeaderboardArgs {},
        )
    }
    fn on_log_claim_member_leaderboard(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext) + Send + 'static,
    ) -> LogClaimMemberLeaderboardCallbackId {
        LogClaimMemberLeaderboardCallbackId(self.imp.on_reducer(
            "log_claim_member_leaderboard",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::LogClaimMemberLeaderboard {},
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx)
            }),
        ))
    }
    fn remove_on_log_claim_member_leaderboard(
        &self,
        callback: LogClaimMemberLeaderboardCallbackId,
    ) {
        self.imp
            .remove_on_reducer("log_claim_member_leaderboard", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `log_claim_member_leaderboard`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_log_claim_member_leaderboard {
    /// Set the call-reducer flags for the reducer `log_claim_member_leaderboard` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn log_claim_member_leaderboard(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_log_claim_member_leaderboard for super::SetReducerFlags {
    fn log_claim_member_leaderboard(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("log_claim_member_leaderboard", flags);
    }
}
