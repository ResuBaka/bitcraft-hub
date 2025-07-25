// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::resource_placement_recipe_desc_type::ResourcePlacementRecipeDesc;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct ImportResourcePlacementRecipeDescArgs {
    pub records: Vec<ResourcePlacementRecipeDesc>,
}

impl From<ImportResourcePlacementRecipeDescArgs> for super::Reducer {
    fn from(args: ImportResourcePlacementRecipeDescArgs) -> Self {
        Self::ImportResourcePlacementRecipeDesc {
            records: args.records,
        }
    }
}

impl __sdk::InModule for ImportResourcePlacementRecipeDescArgs {
    type Module = super::RemoteModule;
}

pub struct ImportResourcePlacementRecipeDescCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `import_resource_placement_recipe_desc`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait import_resource_placement_recipe_desc {
    /// Request that the remote module invoke the reducer `import_resource_placement_recipe_desc` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_import_resource_placement_recipe_desc`] callbacks.
    fn import_resource_placement_recipe_desc(
        &self,
        records: Vec<ResourcePlacementRecipeDesc>,
    ) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `import_resource_placement_recipe_desc`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`ImportResourcePlacementRecipeDescCallbackId`] can be passed to [`Self::remove_on_import_resource_placement_recipe_desc`]
    /// to cancel the callback.
    fn on_import_resource_placement_recipe_desc(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &Vec<ResourcePlacementRecipeDesc>)
        + Send
        + 'static,
    ) -> ImportResourcePlacementRecipeDescCallbackId;
    /// Cancel a callback previously registered by [`Self::on_import_resource_placement_recipe_desc`],
    /// causing it not to run in the future.
    fn remove_on_import_resource_placement_recipe_desc(
        &self,
        callback: ImportResourcePlacementRecipeDescCallbackId,
    );
}

impl import_resource_placement_recipe_desc for super::RemoteReducers {
    fn import_resource_placement_recipe_desc(
        &self,
        records: Vec<ResourcePlacementRecipeDesc>,
    ) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "import_resource_placement_recipe_desc",
            ImportResourcePlacementRecipeDescArgs { records },
        )
    }
    fn on_import_resource_placement_recipe_desc(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &Vec<ResourcePlacementRecipeDesc>)
        + Send
        + 'static,
    ) -> ImportResourcePlacementRecipeDescCallbackId {
        ImportResourcePlacementRecipeDescCallbackId(self.imp.on_reducer(
            "import_resource_placement_recipe_desc",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::ImportResourcePlacementRecipeDesc { records },
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
    fn remove_on_import_resource_placement_recipe_desc(
        &self,
        callback: ImportResourcePlacementRecipeDescCallbackId,
    ) {
        self.imp
            .remove_on_reducer("import_resource_placement_recipe_desc", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `import_resource_placement_recipe_desc`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_import_resource_placement_recipe_desc {
    /// Set the call-reducer flags for the reducer `import_resource_placement_recipe_desc` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn import_resource_placement_recipe_desc(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_import_resource_placement_recipe_desc for super::SetReducerFlags {
    fn import_resource_placement_recipe_desc(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("import_resource_placement_recipe_desc", flags);
    }
}
