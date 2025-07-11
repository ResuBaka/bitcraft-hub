// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::interior_portal_connections_desc_type::InteriorPortalConnectionsDesc;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct ImportInteriorPortalConnectionsDescArgs {
    pub records: Vec<InteriorPortalConnectionsDesc>,
}

impl From<ImportInteriorPortalConnectionsDescArgs> for super::Reducer {
    fn from(args: ImportInteriorPortalConnectionsDescArgs) -> Self {
        Self::ImportInteriorPortalConnectionsDesc {
            records: args.records,
        }
    }
}

impl __sdk::InModule for ImportInteriorPortalConnectionsDescArgs {
    type Module = super::RemoteModule;
}

pub struct ImportInteriorPortalConnectionsDescCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `import_interior_portal_connections_desc`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait import_interior_portal_connections_desc {
    /// Request that the remote module invoke the reducer `import_interior_portal_connections_desc` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_import_interior_portal_connections_desc`] callbacks.
    fn import_interior_portal_connections_desc(
        &self,
        records: Vec<InteriorPortalConnectionsDesc>,
    ) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `import_interior_portal_connections_desc`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`ImportInteriorPortalConnectionsDescCallbackId`] can be passed to [`Self::remove_on_import_interior_portal_connections_desc`]
    /// to cancel the callback.
    fn on_import_interior_portal_connections_desc(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &Vec<InteriorPortalConnectionsDesc>)
        + Send
        + 'static,
    ) -> ImportInteriorPortalConnectionsDescCallbackId;
    /// Cancel a callback previously registered by [`Self::on_import_interior_portal_connections_desc`],
    /// causing it not to run in the future.
    fn remove_on_import_interior_portal_connections_desc(
        &self,
        callback: ImportInteriorPortalConnectionsDescCallbackId,
    );
}

impl import_interior_portal_connections_desc for super::RemoteReducers {
    fn import_interior_portal_connections_desc(
        &self,
        records: Vec<InteriorPortalConnectionsDesc>,
    ) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "import_interior_portal_connections_desc",
            ImportInteriorPortalConnectionsDescArgs { records },
        )
    }
    fn on_import_interior_portal_connections_desc(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &Vec<InteriorPortalConnectionsDesc>)
        + Send
        + 'static,
    ) -> ImportInteriorPortalConnectionsDescCallbackId {
        ImportInteriorPortalConnectionsDescCallbackId(
            self.imp.on_reducer(
                "import_interior_portal_connections_desc",
                Box::new(move |ctx: &super::ReducerEventContext| {
                    let super::ReducerEventContext {
                        event:
                            __sdk::ReducerEvent {
                                reducer:
                                    super::Reducer::ImportInteriorPortalConnectionsDesc { records },
                                ..
                            },
                        ..
                    } = ctx
                    else {
                        unreachable!()
                    };
                    callback(ctx, records)
                }),
            ),
        )
    }
    fn remove_on_import_interior_portal_connections_desc(
        &self,
        callback: ImportInteriorPortalConnectionsDescCallbackId,
    ) {
        self.imp
            .remove_on_reducer("import_interior_portal_connections_desc", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `import_interior_portal_connections_desc`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_import_interior_portal_connections_desc {
    /// Set the call-reducer flags for the reducer `import_interior_portal_connections_desc` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn import_interior_portal_connections_desc(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_import_interior_portal_connections_desc for super::SetReducerFlags {
    fn import_interior_portal_connections_desc(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("import_interior_portal_connections_desc", flags);
    }
}
