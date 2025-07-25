// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::admin_broadcast_type::AdminBroadcast;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct ImportAdminBroadcastArgs {
    pub records: Vec<AdminBroadcast>,
}

impl From<ImportAdminBroadcastArgs> for super::Reducer {
    fn from(args: ImportAdminBroadcastArgs) -> Self {
        Self::ImportAdminBroadcast {
            records: args.records,
        }
    }
}

impl __sdk::InModule for ImportAdminBroadcastArgs {
    type Module = super::RemoteModule;
}

pub struct ImportAdminBroadcastCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `import_admin_broadcast`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait import_admin_broadcast {
    /// Request that the remote module invoke the reducer `import_admin_broadcast` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_import_admin_broadcast`] callbacks.
    fn import_admin_broadcast(&self, records: Vec<AdminBroadcast>) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `import_admin_broadcast`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`ImportAdminBroadcastCallbackId`] can be passed to [`Self::remove_on_import_admin_broadcast`]
    /// to cancel the callback.
    fn on_import_admin_broadcast(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &Vec<AdminBroadcast>) + Send + 'static,
    ) -> ImportAdminBroadcastCallbackId;
    /// Cancel a callback previously registered by [`Self::on_import_admin_broadcast`],
    /// causing it not to run in the future.
    fn remove_on_import_admin_broadcast(&self, callback: ImportAdminBroadcastCallbackId);
}

impl import_admin_broadcast for super::RemoteReducers {
    fn import_admin_broadcast(&self, records: Vec<AdminBroadcast>) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "import_admin_broadcast",
            ImportAdminBroadcastArgs { records },
        )
    }
    fn on_import_admin_broadcast(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &Vec<AdminBroadcast>) + Send + 'static,
    ) -> ImportAdminBroadcastCallbackId {
        ImportAdminBroadcastCallbackId(self.imp.on_reducer(
            "import_admin_broadcast",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::ImportAdminBroadcast { records },
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
    fn remove_on_import_admin_broadcast(&self, callback: ImportAdminBroadcastCallbackId) {
        self.imp
            .remove_on_reducer("import_admin_broadcast", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `import_admin_broadcast`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_import_admin_broadcast {
    /// Set the call-reducer flags for the reducer `import_admin_broadcast` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn import_admin_broadcast(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_import_admin_broadcast for super::SetReducerFlags {
    fn import_admin_broadcast(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("import_admin_broadcast", flags);
    }
}
