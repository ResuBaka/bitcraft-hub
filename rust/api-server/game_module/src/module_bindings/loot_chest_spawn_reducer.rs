// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::loot_chest_spawn_timer_type::LootChestSpawnTimer;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct LootChestSpawnArgs {
    pub timer: LootChestSpawnTimer,
}

impl From<LootChestSpawnArgs> for super::Reducer {
    fn from(args: LootChestSpawnArgs) -> Self {
        Self::LootChestSpawn { timer: args.timer }
    }
}

impl __sdk::InModule for LootChestSpawnArgs {
    type Module = super::RemoteModule;
}

pub struct LootChestSpawnCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `loot_chest_spawn`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait loot_chest_spawn {
    /// Request that the remote module invoke the reducer `loot_chest_spawn` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_loot_chest_spawn`] callbacks.
    fn loot_chest_spawn(&self, timer: LootChestSpawnTimer) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `loot_chest_spawn`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`LootChestSpawnCallbackId`] can be passed to [`Self::remove_on_loot_chest_spawn`]
    /// to cancel the callback.
    fn on_loot_chest_spawn(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &LootChestSpawnTimer) + Send + 'static,
    ) -> LootChestSpawnCallbackId;
    /// Cancel a callback previously registered by [`Self::on_loot_chest_spawn`],
    /// causing it not to run in the future.
    fn remove_on_loot_chest_spawn(&self, callback: LootChestSpawnCallbackId);
}

impl loot_chest_spawn for super::RemoteReducers {
    fn loot_chest_spawn(&self, timer: LootChestSpawnTimer) -> __sdk::Result<()> {
        self.imp
            .call_reducer("loot_chest_spawn", LootChestSpawnArgs { timer })
    }
    fn on_loot_chest_spawn(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &LootChestSpawnTimer) + Send + 'static,
    ) -> LootChestSpawnCallbackId {
        LootChestSpawnCallbackId(self.imp.on_reducer(
            "loot_chest_spawn",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::LootChestSpawn { timer },
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
    fn remove_on_loot_chest_spawn(&self, callback: LootChestSpawnCallbackId) {
        self.imp.remove_on_reducer("loot_chest_spawn", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `loot_chest_spawn`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_loot_chest_spawn {
    /// Set the call-reducer flags for the reducer `loot_chest_spawn` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn loot_chest_spawn(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_loot_chest_spawn for super::SetReducerFlags {
    fn loot_chest_spawn(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("loot_chest_spawn", flags);
    }
}
