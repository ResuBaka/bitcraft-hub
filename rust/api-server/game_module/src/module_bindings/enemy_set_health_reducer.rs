// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::enemy_set_health_request_type::EnemySetHealthRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct EnemySetHealthArgs {
    pub request: EnemySetHealthRequest,
}

impl From<EnemySetHealthArgs> for super::Reducer {
    fn from(args: EnemySetHealthArgs) -> Self {
        Self::EnemySetHealth {
            request: args.request,
        }
    }
}

impl __sdk::InModule for EnemySetHealthArgs {
    type Module = super::RemoteModule;
}

pub struct EnemySetHealthCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `enemy_set_health`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait enemy_set_health {
    /// Request that the remote module invoke the reducer `enemy_set_health` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_enemy_set_health`] callbacks.
    fn enemy_set_health(&self, request: EnemySetHealthRequest) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `enemy_set_health`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`EnemySetHealthCallbackId`] can be passed to [`Self::remove_on_enemy_set_health`]
    /// to cancel the callback.
    fn on_enemy_set_health(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &EnemySetHealthRequest) + Send + 'static,
    ) -> EnemySetHealthCallbackId;
    /// Cancel a callback previously registered by [`Self::on_enemy_set_health`],
    /// causing it not to run in the future.
    fn remove_on_enemy_set_health(&self, callback: EnemySetHealthCallbackId);
}

impl enemy_set_health for super::RemoteReducers {
    fn enemy_set_health(&self, request: EnemySetHealthRequest) -> __sdk::Result<()> {
        self.imp
            .call_reducer("enemy_set_health", EnemySetHealthArgs { request })
    }
    fn on_enemy_set_health(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &EnemySetHealthRequest) + Send + 'static,
    ) -> EnemySetHealthCallbackId {
        EnemySetHealthCallbackId(self.imp.on_reducer(
            "enemy_set_health",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::EnemySetHealth { request },
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
    fn remove_on_enemy_set_health(&self, callback: EnemySetHealthCallbackId) {
        self.imp.remove_on_reducer("enemy_set_health", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `enemy_set_health`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_enemy_set_health {
    /// Set the call-reducer flags for the reducer `enemy_set_health` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn enemy_set_health(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_enemy_set_health for super::SetReducerFlags {
    fn enemy_set_health(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("enemy_set_health", flags);
    }
}
