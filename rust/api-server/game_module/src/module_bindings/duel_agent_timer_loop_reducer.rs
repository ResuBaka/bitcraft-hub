// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::duel_agent_timer_type::DuelAgentTimer;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct DuelAgentTimerLoopArgs {
    pub timer: DuelAgentTimer,
}

impl From<DuelAgentTimerLoopArgs> for super::Reducer {
    fn from(args: DuelAgentTimerLoopArgs) -> Self {
        Self::DuelAgentTimerLoop { timer: args.timer }
    }
}

impl __sdk::InModule for DuelAgentTimerLoopArgs {
    type Module = super::RemoteModule;
}

pub struct DuelAgentTimerLoopCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `duel_agent_timer_loop`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait duel_agent_timer_loop {
    /// Request that the remote module invoke the reducer `duel_agent_timer_loop` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_duel_agent_timer_loop`] callbacks.
    fn duel_agent_timer_loop(&self, timer: DuelAgentTimer) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `duel_agent_timer_loop`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`DuelAgentTimerLoopCallbackId`] can be passed to [`Self::remove_on_duel_agent_timer_loop`]
    /// to cancel the callback.
    fn on_duel_agent_timer_loop(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &DuelAgentTimer) + Send + 'static,
    ) -> DuelAgentTimerLoopCallbackId;
    /// Cancel a callback previously registered by [`Self::on_duel_agent_timer_loop`],
    /// causing it not to run in the future.
    fn remove_on_duel_agent_timer_loop(&self, callback: DuelAgentTimerLoopCallbackId);
}

impl duel_agent_timer_loop for super::RemoteReducers {
    fn duel_agent_timer_loop(&self, timer: DuelAgentTimer) -> __sdk::Result<()> {
        self.imp
            .call_reducer("duel_agent_timer_loop", DuelAgentTimerLoopArgs { timer })
    }
    fn on_duel_agent_timer_loop(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &DuelAgentTimer) + Send + 'static,
    ) -> DuelAgentTimerLoopCallbackId {
        DuelAgentTimerLoopCallbackId(self.imp.on_reducer(
            "duel_agent_timer_loop",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::DuelAgentTimerLoop { timer },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx, timer)
            }),
        ))
    }
    fn remove_on_duel_agent_timer_loop(&self, callback: DuelAgentTimerLoopCallbackId) {
        self.imp
            .remove_on_reducer("duel_agent_timer_loop", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `duel_agent_timer_loop`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_duel_agent_timer_loop {
    /// Set the call-reducer flags for the reducer `duel_agent_timer_loop` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn duel_agent_timer_loop(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_duel_agent_timer_loop for super::SetReducerFlags {
    fn duel_agent_timer_loop(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("duel_agent_timer_loop", flags);
    }
}
