// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::resource_count_type::ResourceCount;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct ImportResourceCountArgs {
    pub records: Vec<ResourceCount>,
}

impl From<ImportResourceCountArgs> for super::Reducer {
    fn from(args: ImportResourceCountArgs) -> Self {
        Self::ImportResourceCount {
            records: args.records,
        }
    }
}

impl __sdk::InModule for ImportResourceCountArgs {
    type Module = super::RemoteModule;
}

pub struct ImportResourceCountCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `import_resource_count`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait import_resource_count {
    /// Request that the remote module invoke the reducer `import_resource_count` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_import_resource_count`] callbacks.
    fn import_resource_count(&self, records: Vec<ResourceCount>) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `import_resource_count`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`ImportResourceCountCallbackId`] can be passed to [`Self::remove_on_import_resource_count`]
    /// to cancel the callback.
    fn on_import_resource_count(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &Vec<ResourceCount>) + Send + 'static,
    ) -> ImportResourceCountCallbackId;
    /// Cancel a callback previously registered by [`Self::on_import_resource_count`],
    /// causing it not to run in the future.
    fn remove_on_import_resource_count(&self, callback: ImportResourceCountCallbackId);
}

impl import_resource_count for super::RemoteReducers {
    fn import_resource_count(&self, records: Vec<ResourceCount>) -> __sdk::Result<()> {
        self.imp
            .call_reducer("import_resource_count", ImportResourceCountArgs { records })
    }
    fn on_import_resource_count(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &Vec<ResourceCount>) + Send + 'static,
    ) -> ImportResourceCountCallbackId {
        ImportResourceCountCallbackId(self.imp.on_reducer(
            "import_resource_count",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::ImportResourceCount { records },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx, records)
            }),
        ))
    }
    fn remove_on_import_resource_count(&self, callback: ImportResourceCountCallbackId) {
        self.imp
            .remove_on_reducer("import_resource_count", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `import_resource_count`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_import_resource_count {
    /// Set the call-reducer flags for the reducer `import_resource_count` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn import_resource_count(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_import_resource_count for super::SetReducerFlags {
    fn import_resource_count(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("import_resource_count", flags);
    }
}
