// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::user_state_type::UserState;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct ImportUserStateArgs {
    pub records: Vec<UserState>,
}

impl From<ImportUserStateArgs> for super::Reducer {
    fn from(args: ImportUserStateArgs) -> Self {
        Self::ImportUserState {
            records: args.records,
        }
    }
}

impl __sdk::InModule for ImportUserStateArgs {
    type Module = super::RemoteModule;
}

pub struct ImportUserStateCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `import_user_state`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait import_user_state {
    /// Request that the remote module invoke the reducer `import_user_state` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_import_user_state`] callbacks.
    fn import_user_state(&self, records: Vec<UserState>) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `import_user_state`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`ImportUserStateCallbackId`] can be passed to [`Self::remove_on_import_user_state`]
    /// to cancel the callback.
    fn on_import_user_state(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &Vec<UserState>) + Send + 'static,
    ) -> ImportUserStateCallbackId;
    /// Cancel a callback previously registered by [`Self::on_import_user_state`],
    /// causing it not to run in the future.
    fn remove_on_import_user_state(&self, callback: ImportUserStateCallbackId);
}

impl import_user_state for super::RemoteReducers {
    fn import_user_state(&self, records: Vec<UserState>) -> __sdk::Result<()> {
        self.imp
            .call_reducer("import_user_state", ImportUserStateArgs { records })
    }
    fn on_import_user_state(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &Vec<UserState>) + Send + 'static,
    ) -> ImportUserStateCallbackId {
        ImportUserStateCallbackId(self.imp.on_reducer(
            "import_user_state",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::ImportUserState { records },
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
    fn remove_on_import_user_state(&self, callback: ImportUserStateCallbackId) {
        self.imp.remove_on_reducer("import_user_state", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `import_user_state`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_import_user_state {
    /// Set the call-reducer flags for the reducer `import_user_state` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn import_user_state(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_import_user_state for super::SetReducerFlags {
    fn import_user_state(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("import_user_state", flags);
    }
}
