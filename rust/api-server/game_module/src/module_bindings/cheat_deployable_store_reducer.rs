// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct CheatDeployableStoreArgs {
    pub deployable_entity_id: u64,
}

impl From<CheatDeployableStoreArgs> for super::Reducer {
    fn from(args: CheatDeployableStoreArgs) -> Self {
        Self::CheatDeployableStore {
            deployable_entity_id: args.deployable_entity_id,
        }
    }
}

impl __sdk::InModule for CheatDeployableStoreArgs {
    type Module = super::RemoteModule;
}

pub struct CheatDeployableStoreCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `cheat_deployable_store`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait cheat_deployable_store {
    /// Request that the remote module invoke the reducer `cheat_deployable_store` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_cheat_deployable_store`] callbacks.
    fn cheat_deployable_store(&self, deployable_entity_id: u64) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `cheat_deployable_store`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`CheatDeployableStoreCallbackId`] can be passed to [`Self::remove_on_cheat_deployable_store`]
    /// to cancel the callback.
    fn on_cheat_deployable_store(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &u64) + Send + 'static,
    ) -> CheatDeployableStoreCallbackId;
    /// Cancel a callback previously registered by [`Self::on_cheat_deployable_store`],
    /// causing it not to run in the future.
    fn remove_on_cheat_deployable_store(&self, callback: CheatDeployableStoreCallbackId);
}

impl cheat_deployable_store for super::RemoteReducers {
    fn cheat_deployable_store(&self, deployable_entity_id: u64) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "cheat_deployable_store",
            CheatDeployableStoreArgs {
                deployable_entity_id,
            },
        )
    }
    fn on_cheat_deployable_store(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &u64) + Send + 'static,
    ) -> CheatDeployableStoreCallbackId {
        CheatDeployableStoreCallbackId(self.imp.on_reducer(
            "cheat_deployable_store",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer:
                                super::Reducer::CheatDeployableStore {
                                    deployable_entity_id,
                                },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx, deployable_entity_id)
            }),
        ))
    }
    fn remove_on_cheat_deployable_store(&self, callback: CheatDeployableStoreCallbackId) {
        self.imp
            .remove_on_reducer("cheat_deployable_store", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `cheat_deployable_store`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_cheat_deployable_store {
    /// Set the call-reducer flags for the reducer `cheat_deployable_store` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn cheat_deployable_store(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_cheat_deployable_store for super::SetReducerFlags {
    fn cheat_deployable_store(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("cheat_deployable_store", flags);
    }
}
