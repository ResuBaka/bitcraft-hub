// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct AdminCountInventoryItemsArgs {
    pub item_id: i32,
    pub limit: u32,
}

impl From<AdminCountInventoryItemsArgs> for super::Reducer {
    fn from(args: AdminCountInventoryItemsArgs) -> Self {
        Self::AdminCountInventoryItems {
            item_id: args.item_id,
            limit: args.limit,
        }
    }
}

impl __sdk::InModule for AdminCountInventoryItemsArgs {
    type Module = super::RemoteModule;
}

pub struct AdminCountInventoryItemsCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `admin_count_inventory_items`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait admin_count_inventory_items {
    /// Request that the remote module invoke the reducer `admin_count_inventory_items` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_admin_count_inventory_items`] callbacks.
    fn admin_count_inventory_items(&self, item_id: i32, limit: u32) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `admin_count_inventory_items`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`AdminCountInventoryItemsCallbackId`] can be passed to [`Self::remove_on_admin_count_inventory_items`]
    /// to cancel the callback.
    fn on_admin_count_inventory_items(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &i32, &u32) + Send + 'static,
    ) -> AdminCountInventoryItemsCallbackId;
    /// Cancel a callback previously registered by [`Self::on_admin_count_inventory_items`],
    /// causing it not to run in the future.
    fn remove_on_admin_count_inventory_items(&self, callback: AdminCountInventoryItemsCallbackId);
}

impl admin_count_inventory_items for super::RemoteReducers {
    fn admin_count_inventory_items(&self, item_id: i32, limit: u32) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "admin_count_inventory_items",
            AdminCountInventoryItemsArgs { item_id, limit },
        )
    }
    fn on_admin_count_inventory_items(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &i32, &u32) + Send + 'static,
    ) -> AdminCountInventoryItemsCallbackId {
        AdminCountInventoryItemsCallbackId(self.imp.on_reducer(
            "admin_count_inventory_items",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::AdminCountInventoryItems { item_id, limit },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx, item_id, limit)
            }),
        ))
    }
    fn remove_on_admin_count_inventory_items(&self, callback: AdminCountInventoryItemsCallbackId) {
        self.imp
            .remove_on_reducer("admin_count_inventory_items", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `admin_count_inventory_items`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_admin_count_inventory_items {
    /// Set the call-reducer flags for the reducer `admin_count_inventory_items` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn admin_count_inventory_items(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_admin_count_inventory_items for super::SetReducerFlags {
    fn admin_count_inventory_items(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("admin_count_inventory_items", flags);
    }
}
