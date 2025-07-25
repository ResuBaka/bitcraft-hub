// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::knowledge_stat_modifier_desc_type::KnowledgeStatModifierDesc;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct StageKnowledgeStatModifierDescArgs {
    pub records: Vec<KnowledgeStatModifierDesc>,
}

impl From<StageKnowledgeStatModifierDescArgs> for super::Reducer {
    fn from(args: StageKnowledgeStatModifierDescArgs) -> Self {
        Self::StageKnowledgeStatModifierDesc {
            records: args.records,
        }
    }
}

impl __sdk::InModule for StageKnowledgeStatModifierDescArgs {
    type Module = super::RemoteModule;
}

pub struct StageKnowledgeStatModifierDescCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `stage_knowledge_stat_modifier_desc`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait stage_knowledge_stat_modifier_desc {
    /// Request that the remote module invoke the reducer `stage_knowledge_stat_modifier_desc` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_stage_knowledge_stat_modifier_desc`] callbacks.
    fn stage_knowledge_stat_modifier_desc(
        &self,
        records: Vec<KnowledgeStatModifierDesc>,
    ) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `stage_knowledge_stat_modifier_desc`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`StageKnowledgeStatModifierDescCallbackId`] can be passed to [`Self::remove_on_stage_knowledge_stat_modifier_desc`]
    /// to cancel the callback.
    fn on_stage_knowledge_stat_modifier_desc(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &Vec<KnowledgeStatModifierDesc>)
        + Send
        + 'static,
    ) -> StageKnowledgeStatModifierDescCallbackId;
    /// Cancel a callback previously registered by [`Self::on_stage_knowledge_stat_modifier_desc`],
    /// causing it not to run in the future.
    fn remove_on_stage_knowledge_stat_modifier_desc(
        &self,
        callback: StageKnowledgeStatModifierDescCallbackId,
    );
}

impl stage_knowledge_stat_modifier_desc for super::RemoteReducers {
    fn stage_knowledge_stat_modifier_desc(
        &self,
        records: Vec<KnowledgeStatModifierDesc>,
    ) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "stage_knowledge_stat_modifier_desc",
            StageKnowledgeStatModifierDescArgs { records },
        )
    }
    fn on_stage_knowledge_stat_modifier_desc(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &Vec<KnowledgeStatModifierDesc>)
        + Send
        + 'static,
    ) -> StageKnowledgeStatModifierDescCallbackId {
        StageKnowledgeStatModifierDescCallbackId(self.imp.on_reducer(
            "stage_knowledge_stat_modifier_desc",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::StageKnowledgeStatModifierDesc { records },
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
    fn remove_on_stage_knowledge_stat_modifier_desc(
        &self,
        callback: StageKnowledgeStatModifierDescCallbackId,
    ) {
        self.imp
            .remove_on_reducer("stage_knowledge_stat_modifier_desc", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `stage_knowledge_stat_modifier_desc`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_stage_knowledge_stat_modifier_desc {
    /// Set the call-reducer flags for the reducer `stage_knowledge_stat_modifier_desc` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn stage_knowledge_stat_modifier_desc(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_stage_knowledge_stat_modifier_desc for super::SetReducerFlags {
    fn stage_knowledge_stat_modifier_desc(&self, flags: __ws::CallReducerFlags) {
        self.imp
            .set_call_reducer_flags("stage_knowledge_stat_modifier_desc", flags);
    }
}
