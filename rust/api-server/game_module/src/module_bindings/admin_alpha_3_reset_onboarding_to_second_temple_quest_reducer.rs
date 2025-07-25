// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub(super) struct AdminAlpha3ResetOnboardingToSecondTempleQuestArgs {
    pub entity_id: u64,
}

impl From<AdminAlpha3ResetOnboardingToSecondTempleQuestArgs> for super::Reducer {
    fn from(args: AdminAlpha3ResetOnboardingToSecondTempleQuestArgs) -> Self {
        Self::AdminAlpha3ResetOnboardingToSecondTempleQuest {
            entity_id: args.entity_id,
        }
    }
}

impl __sdk::InModule for AdminAlpha3ResetOnboardingToSecondTempleQuestArgs {
    type Module = super::RemoteModule;
}

pub struct AdminAlpha3ResetOnboardingToSecondTempleQuestCallbackId(__sdk::CallbackId);

#[allow(non_camel_case_types)]
/// Extension trait for access to the reducer `admin_alpha3_reset_onboarding_to_second_temple_quest`.
///
/// Implemented for [`super::RemoteReducers`].
pub trait admin_alpha_3_reset_onboarding_to_second_temple_quest {
    /// Request that the remote module invoke the reducer `admin_alpha3_reset_onboarding_to_second_temple_quest` to run as soon as possible.
    ///
    /// This method returns immediately, and errors only if we are unable to send the request.
    /// The reducer will run asynchronously in the future,
    ///  and its status can be observed by listening for [`Self::on_admin_alpha_3_reset_onboarding_to_second_temple_quest`] callbacks.
    fn admin_alpha_3_reset_onboarding_to_second_temple_quest(
        &self,
        entity_id: u64,
    ) -> __sdk::Result<()>;
    /// Register a callback to run whenever we are notified of an invocation of the reducer `admin_alpha3_reset_onboarding_to_second_temple_quest`.
    ///
    /// Callbacks should inspect the [`__sdk::ReducerEvent`] contained in the [`super::ReducerEventContext`]
    /// to determine the reducer's status.
    ///
    /// The returned [`AdminAlpha3ResetOnboardingToSecondTempleQuestCallbackId`] can be passed to [`Self::remove_on_admin_alpha_3_reset_onboarding_to_second_temple_quest`]
    /// to cancel the callback.
    fn on_admin_alpha_3_reset_onboarding_to_second_temple_quest(
        &self,
        callback: impl FnMut(&super::ReducerEventContext, &u64) + Send + 'static,
    ) -> AdminAlpha3ResetOnboardingToSecondTempleQuestCallbackId;
    /// Cancel a callback previously registered by [`Self::on_admin_alpha_3_reset_onboarding_to_second_temple_quest`],
    /// causing it not to run in the future.
    fn remove_on_admin_alpha_3_reset_onboarding_to_second_temple_quest(
        &self,
        callback: AdminAlpha3ResetOnboardingToSecondTempleQuestCallbackId,
    );
}

impl admin_alpha_3_reset_onboarding_to_second_temple_quest for super::RemoteReducers {
    fn admin_alpha_3_reset_onboarding_to_second_temple_quest(
        &self,
        entity_id: u64,
    ) -> __sdk::Result<()> {
        self.imp.call_reducer(
            "admin_alpha3_reset_onboarding_to_second_temple_quest",
            AdminAlpha3ResetOnboardingToSecondTempleQuestArgs { entity_id },
        )
    }
    fn on_admin_alpha_3_reset_onboarding_to_second_temple_quest(
        &self,
        mut callback: impl FnMut(&super::ReducerEventContext, &u64) + Send + 'static,
    ) -> AdminAlpha3ResetOnboardingToSecondTempleQuestCallbackId {
        AdminAlpha3ResetOnboardingToSecondTempleQuestCallbackId(self.imp.on_reducer(
            "admin_alpha3_reset_onboarding_to_second_temple_quest",
            Box::new(move |ctx: &super::ReducerEventContext| {
                let super::ReducerEventContext {
                    event:
                        __sdk::ReducerEvent {
                            reducer:
                                super::Reducer::AdminAlpha3ResetOnboardingToSecondTempleQuest {
                                    entity_id,
                                },
                            ..
                        },
                    ..
                } = ctx
                else {
                    unreachable!()
                };
                callback(ctx, entity_id)
            }),
        ))
    }
    fn remove_on_admin_alpha_3_reset_onboarding_to_second_temple_quest(
        &self,
        callback: AdminAlpha3ResetOnboardingToSecondTempleQuestCallbackId,
    ) {
        self.imp.remove_on_reducer(
            "admin_alpha3_reset_onboarding_to_second_temple_quest",
            callback.0,
        )
    }
}

#[allow(non_camel_case_types)]
#[doc(hidden)]
/// Extension trait for setting the call-flags for the reducer `admin_alpha3_reset_onboarding_to_second_temple_quest`.
///
/// Implemented for [`super::SetReducerFlags`].
///
/// This type is currently unstable and may be removed without a major version bump.
pub trait set_flags_for_admin_alpha_3_reset_onboarding_to_second_temple_quest {
    /// Set the call-reducer flags for the reducer `admin_alpha3_reset_onboarding_to_second_temple_quest` to `flags`.
    ///
    /// This type is currently unstable and may be removed without a major version bump.
    fn admin_alpha_3_reset_onboarding_to_second_temple_quest(&self, flags: __ws::CallReducerFlags);
}

impl set_flags_for_admin_alpha_3_reset_onboarding_to_second_temple_quest
    for super::SetReducerFlags
{
    fn admin_alpha_3_reset_onboarding_to_second_temple_quest(&self, flags: __ws::CallReducerFlags) {
        self.imp.set_call_reducer_flags(
            "admin_alpha3_reset_onboarding_to_second_temple_quest",
            flags,
        );
    }
}
