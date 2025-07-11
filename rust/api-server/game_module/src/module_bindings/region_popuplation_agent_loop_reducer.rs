// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::region_population_loop_timer_type::RegionPopulationLoopTimer;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct RegionPopuplationAgentLoopArgs {
    pub timer: RegionPopulationLoopTimer,
}

impl From<RegionPopuplationAgentLoopArgs> for super::Reducer {
    fn from(args: RegionPopuplationAgentLoopArgs) -> Self {
        Self::RegionPopuplationAgentLoop { timer: args.timer }
    }
}

impl __sdk::InModule for RegionPopuplationAgentLoopArgs {
    type Module = super::RemoteModule;
}

pub struct RegionPopuplationAgentLoopCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `region_popuplation_agent_loop`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait region_popuplation_agent_loop {
    /// Request that the remote module invoke the reducer `region_popuplation_agent_loop` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_region_popuplation_agent_loop`] callbacks.
    fn region_popuplation_agent_loop(&self, timer: RegionPopulationLoopTimer) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `region_popuplation_agent_loop`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`RegionPopuplationAgentLoopCallbackId`] can be passed to [`Self::remove_on_region_popuplation_agent_loop`]
    /// to cancel the callback.
    fn on_region_popuplation_agent_loop(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &RegionPopulationLoopTimer) + Send + 'static,
    ) -> RegionPopuplationAgentLoopCallbackId;
    /// Cancel a callback previously registered by [`Self::on_region_popuplation_agent_loop`],
    /// causing it not to run in the future.
    fn remove_on_region_popuplation_agent_loop(
        &self,
        callback: RegionPopuplationAgentLoopCallbackId,
    );
}

impl region_popuplation_agent_loop for super::RemoteReducers {
    fn region_popuplation_agent_loop(&self, timer: RegionPopulationLoopTimer) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "region_popuplation_agent_loop",
            RegionPopuplationAgentLoopArgs { timer },
        )
    }
    fn on_region_popuplation_agent_loop(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &RegionPopulationLoopTimer)
        + Send
        + 'static,
    ) -> RegionPopuplationAgentLoopCallbackId {
        RegionPopuplationAgentLoopCallbackId(self.imp.on_reducer(
            "region_popuplation_agent_loop",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::RegionPopuplationAgentLoop { timer },
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
    fn remove_on_region_popuplation_agent_loop(
        &self,
        callback: RegionPopuplationAgentLoopCallbackId,
    ) {
        self.imp
            .remove_on_reducer("region_popuplation_agent_loop", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `region_popuplation_agent_loop`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_region_popuplation_agent_loop {
    /// Set the call-reducer flags for the reducer `region_popuplation_agent_loop` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn region_popuplation_agent_loop(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_region_popuplation_agent_loop for super::SetReducerFlags {
    fn region_popuplation_agent_loop(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("region_popuplation_agent_loop", flags);
    }
}
