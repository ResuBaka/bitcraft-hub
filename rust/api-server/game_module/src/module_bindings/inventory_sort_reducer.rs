// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct InventorySortArgs {
    pub target_entity_id: u64,
}

impl From<InventorySortArgs> for super::Reducer {
    fn from(args: InventorySortArgs) -> Self {
        Self::InventorySort {
            target_entity_id: args.target_entity_id,
        }
    }
}

impl __sdk::InModule for InventorySortArgs {
    type Module = super::RemoteModule;
}

pub struct InventorySortCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `inventory_sort`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait inventory_sort {
    /// Request that the remote module invoke the reducer `inventory_sort` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_inventory_sort`] callbacks.
    fn inventory_sort(&self, target_entity_id: u64) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `inventory_sort`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`InventorySortCallbackId`] can be passed to [`Self::remove_on_inventory_sort`]
    /// to cancel the callback.
    fn on_inventory_sort(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &u64) + Send + 'static,
    ) -> InventorySortCallbackId;
    /// Cancel a callback previously registered by [`Self::on_inventory_sort`],
    /// causing it not to run in the future.
    fn remove_on_inventory_sort(&self, callback: InventorySortCallbackId);
}

impl inventory_sort for super::RemoteReducers {
    fn inventory_sort(&self, target_entity_id: u64) -> __sdk::Result<()> {
        self.imp
            .call_reducer("inventory_sort", InventorySortArgs { target_entity_id })
    }
    fn on_inventory_sort(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &u64) + Send + 'static,
    ) -> InventorySortCallbackId {
        InventorySortCallbackId(self.imp.on_reducer(
            "inventory_sort",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::InventorySort { target_entity_id },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx, target_entity_id)
            }),
        ))
    }
    fn remove_on_inventory_sort(&self, callback: InventorySortCallbackId) {
        self.imp.remove_on_reducer("inventory_sort", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `inventory_sort`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_inventory_sort {
    /// Set the call-reducer flags for the reducer `inventory_sort` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn inventory_sort(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_inventory_sort for super::SetReducerFlags {
    fn inventory_sort(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("inventory_sort", flags);
    }
}
