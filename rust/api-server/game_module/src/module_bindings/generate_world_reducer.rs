// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::world_gen_world_definition_type::WorldGenWorldDefinition;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct GenerateWorldArgs {
    pub world_definition: WorldGenWorldDefinition,
}

impl From<GenerateWorldArgs> for super::Reducer {
    fn from(args: GenerateWorldArgs) -> Self {
        Self::GenerateWorld {
            world_definition: args.world_definition,
        }
    }
}

impl __sdk::InModule for GenerateWorldArgs {
    type Module = super::RemoteModule;
}

pub struct GenerateWorldCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `generate_world`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait generate_world {
    /// Request that the remote module invoke the reducer `generate_world` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_generate_world`] callbacks.
    fn generate_world(&self, world_definition: WorldGenWorldDefinition) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `generate_world`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`GenerateWorldCallbackId`] can be passed to [`Self::remove_on_generate_world`]
    /// to cancel the callback.
    fn on_generate_world(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &WorldGenWorldDefinition) + Send + 'static,
    ) -> GenerateWorldCallbackId;
    /// Cancel a callback previously registered by [`Self::on_generate_world`],
    /// causing it not to run in the future.
    fn remove_on_generate_world(&self, callback: GenerateWorldCallbackId);
}

impl generate_world for super::RemoteReducers {
    fn generate_world(&self, world_definition: WorldGenWorldDefinition) -> __sdk::Result<()> {
        self.imp
            .call_reducer("generate_world", GenerateWorldArgs { world_definition })
    }
    fn on_generate_world(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &WorldGenWorldDefinition) + Send + 'static,
    ) -> GenerateWorldCallbackId {
        GenerateWorldCallbackId(self.imp.on_reducer(
            "generate_world",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::GenerateWorld { world_definition },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx, world_definition)
            }),
        ))
    }
    fn remove_on_generate_world(&self, callback: GenerateWorldCallbackId) {
        self.imp.remove_on_reducer("generate_world", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `generate_world`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_generate_world {
    /// Set the call-reducer flags for the reducer `generate_world` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn generate_world(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_generate_world for super::SetReducerFlags {
    fn generate_world(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("generate_world", flags);
    }
}
