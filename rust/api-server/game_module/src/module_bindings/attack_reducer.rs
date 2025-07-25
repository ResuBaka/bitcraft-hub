// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::entity_attack_request_type::EntityAttackRequest;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct AttackArgs {
    pub request: EntityAttackRequest,
}

impl From<AttackArgs> for super::Reducer {
    fn from(args: AttackArgs) -> Self {
        Self::Attack {
            request: args.request,
        }
    }
}

impl __sdk::InModule for AttackArgs {
    type Module = super::RemoteModule;
}

pub struct AttackCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `attack`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait attack {
    /// Request that the remote module invoke the reducer `attack` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_attack`] callbacks.
    fn attack(&self, request: EntityAttackRequest) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `attack`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`AttackCallbackId`] can be passed to [`Self::remove_on_attack`]
    /// to cancel the callback.
    fn on_attack(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &EntityAttackRequest) + Send + 'static,
    ) -> AttackCallbackId;
    /// Cancel a callback previously registered by [`Self::on_attack`],
    /// causing it not to run in the future.
    fn remove_on_attack(&self, callback: AttackCallbackId);
}

impl attack for super::RemoteReducers {
    fn attack(&self, request: EntityAttackRequest) -> __sdk::Result<()> {
        self.imp.call_reducer("attack", AttackArgs { request })
    }
    fn on_attack(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &EntityAttackRequest) + Send + 'static,
    ) -> AttackCallbackId {
        AttackCallbackId(self.imp.on_reducer(
            "attack",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer: super::Reducer::Attack { request },
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
    fn remove_on_attack(&self, callback: AttackCallbackId) {
        self.imp.remove_on_reducer("attack", callback.0)
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `attack`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_attack {
    /// Set the call-reducer flags for the reducer `attack` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn attack(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_attack for super::SetReducerFlags {
    fn attack(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags("attack", flags);
    }
}
