// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::player_item_convert_request_type::PlayerItemConvertRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct ItemConvertArgs {
    pub request: PlayerItemConvertRequest,
}

impl From<ItemConvertArgs> for super::Reducer {
    fn from(args: ItemConvertArgs) -> Self {
        Self::ItemConvert {
            request: args.request,
        }
    }
}

impl __sdk::InModule for ItemConvertArgs {
    type Module = super::RemoteModule;
}

pub struct ItemConvertCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `item_convert`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait item_convert {
    /// Request that the remote module invoke the reducer `item_convert` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_item_convert`] callbacks.
    fn item_convert(&self, request: PlayerItemConvertRequest) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `item_convert`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`ItemConvertCallbackId`] can be passed to [`Self::remove_on_item_convert`]
    /// to cancel the callback.
    fn on_item_convert(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &PlayerItemConvertRequest) + Send + 'static,
    ) -> ItemConvertCallbackId;
    /// Cancel a callback previously registered by [`Self::on_item_convert`],
    /// causing it not to run in the future.
    fn remove_on_item_convert(&self, callback: ItemConvertCallbackId);
}

impl item_convert for super::RemoteReducers {
    fn item_convert(&self, request: PlayerItemConvertRequest) -> __sdk::Result<()> {
        self.imp
            .call_reducer("item_convert", ItemConvertArgs { request })
    }
    fn on_item_convert(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &PlayerItemConvertRequest)
        + Send
        + 'static,
    ) -> ItemConvertCallbackId {
        ItemConvertCallbackId(self.imp.on_reducer(
            "item_convert",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::ItemConvert { request },
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
    fn remove_on_item_convert(&self, callback: ItemConvertCallbackId) {
        self.imp.remove_on_reducer("item_convert", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `item_convert`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_item_convert {
    /// Set the call-reducer flags for the reducer `item_convert` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn item_convert(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_item_convert for super::SetReducerFlags {
    fn item_convert(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("item_convert", flags);
    }
}
