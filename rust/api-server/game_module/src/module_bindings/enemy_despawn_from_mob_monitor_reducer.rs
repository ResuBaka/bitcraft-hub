// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct EnemyDespawnFromMobMonitorArgs {
    pub enemy_entity_id: u64,
}

impl From<EnemyDespawnFromMobMonitorArgs> for super::Reducer {
    fn from(args: EnemyDespawnFromMobMonitorArgs) -> Self {
        Self::EnemyDespawnFromMobMonitor {
            enemy_entity_id: args.enemy_entity_id,
        }
    }
}

impl __sdk::InModule for EnemyDespawnFromMobMonitorArgs {
    type Module = super::RemoteModule;
}

pub struct EnemyDespawnFromMobMonitorCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `enemy_despawn_from_mob_monitor`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait enemy_despawn_from_mob_monitor {
    /// Request that the remote module invoke the reducer `enemy_despawn_from_mob_monitor` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_enemy_despawn_from_mob_monitor`] callbacks.
    fn enemy_despawn_from_mob_monitor(&self, enemy_entity_id: u64) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `enemy_despawn_from_mob_monitor`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`EnemyDespawnFromMobMonitorCallbackId`] can be passed to [`Self::remove_on_enemy_despawn_from_mob_monitor`]
    /// to cancel the callback.
    fn on_enemy_despawn_from_mob_monitor(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &u64) + Send + 'static,
    ) -> EnemyDespawnFromMobMonitorCallbackId;
    /// Cancel a callback previously registered by [`Self::on_enemy_despawn_from_mob_monitor`],
    /// causing it not to run in the future.
    fn remove_on_enemy_despawn_from_mob_monitor(
        &self,
        callback: EnemyDespawnFromMobMonitorCallbackId,
    );
}

impl enemy_despawn_from_mob_monitor for super::RemoteReducers {
    fn enemy_despawn_from_mob_monitor(&self, enemy_entity_id: u64) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "enemy_despawn_from_mob_monitor",
            EnemyDespawnFromMobMonitorArgs { enemy_entity_id },
        )
    }
    fn on_enemy_despawn_from_mob_monitor(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &u64) + Send + 'static,
    ) -> EnemyDespawnFromMobMonitorCallbackId {
        EnemyDespawnFromMobMonitorCallbackId(self.imp.on_reducer(
            "enemy_despawn_from_mob_monitor",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::EnemyDespawnFromMobMonitor { enemy_entity_id },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx, enemy_entity_id)
            }),
        ))
    }
    fn remove_on_enemy_despawn_from_mob_monitor(
        &self,
        callback: EnemyDespawnFromMobMonitorCallbackId,
    ) {
        self.imp
            .remove_on_reducer("enemy_despawn_from_mob_monitor", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `enemy_despawn_from_mob_monitor`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_enemy_despawn_from_mob_monitor {
    /// Set the call-reducer flags for the reducer `enemy_despawn_from_mob_monitor` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn enemy_despawn_from_mob_monitor(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_enemy_despawn_from_mob_monitor for super::SetReducerFlags {
    fn enemy_despawn_from_mob_monitor(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("enemy_despawn_from_mob_monitor", flags);
    }
}
