// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct AdminCollapseRuinArgs {
    pub ruin_building_entity_id: u64,
}

impl From<AdminCollapseRuinArgs> for super::Reducer {
    fn from(args: AdminCollapseRuinArgs) -> Self {
        Self::AdminCollapseRuin {
            ruin_building_entity_id: args.ruin_building_entity_id,
        }
    }
}

impl __sdk::InModule for AdminCollapseRuinArgs {
    type Module = super::RemoteModule;
}

pub struct AdminCollapseRuinCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `admin_collapse_ruin`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait admin_collapse_ruin {
    /// Request that the remote module invoke the reducer `admin_collapse_ruin` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_admin_collapse_ruin`] callbacks.
    fn admin_collapse_ruin(&self, ruin_building_entity_id: u64) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `admin_collapse_ruin`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`AdminCollapseRuinCallbackId`] can be passed to [`Self::remove_on_admin_collapse_ruin`]
    /// to cancel the callback.
    fn on_admin_collapse_ruin(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &u64) + Send + 'static,
    ) -> AdminCollapseRuinCallbackId;
    /// Cancel a callback previously registered by [`Self::on_admin_collapse_ruin`],
    /// causing it not to run in the future.
    fn remove_on_admin_collapse_ruin(&self, callback: AdminCollapseRuinCallbackId);
}

impl admin_collapse_ruin for super::RemoteReducers {
    fn admin_collapse_ruin(&self, ruin_building_entity_id: u64) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "admin_collapse_ruin",
            AdminCollapseRuinArgs {
                ruin_building_entity_id,
            },
        )
    }
    fn on_admin_collapse_ruin(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &u64) + Send + 'static,
    ) -> AdminCollapseRuinCallbackId {
        AdminCollapseRuinCallbackId(self.imp.on_reducer(
            "admin_collapse_ruin",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer:
                                super::Reducer::AdminCollapseRuin {
                                    ruin_building_entity_id,
                                },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx, ruin_building_entity_id)
            }),
        ))
    }
    fn remove_on_admin_collapse_ruin(&self, callback: AdminCollapseRuinCallbackId) {
        self.imp
            .remove_on_reducer("admin_collapse_ruin", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `admin_collapse_ruin`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_admin_collapse_ruin {
    /// Set the call-reducer flags for the reducer `admin_collapse_ruin` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn admin_collapse_ruin(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_admin_collapse_ruin for super::SetReducerFlags {
    fn admin_collapse_ruin(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("admin_collapse_ruin", flags);
    }
}
