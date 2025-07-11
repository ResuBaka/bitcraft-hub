// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::extract_outcome_state_type::ExtractOutcomeState;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `extract_outcome_state`.
///
/// Obtain a handle from the [`ExtractOutcomeStateTableAccess::extract_outcome_state`] method on [`super::RemoteTables`],
/// like `ctx.db.extract_outcome_state()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.extract_outcome_state().on_insert(...)`.
pub struct ExtractOutcomeStateTableHandle<'ctx> {
    imp: __sdk::TableHandle<ExtractOutcomeState>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `extract_outcome_state`.
///
/// Implemented for [`super::RemoteTables`].
pub trait ExtractOutcomeStateTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`ExtractOutcomeStateTableHandle`], which mediates access to the table `extract_outcome_state`.
    fn extract_outcome_state(&self) -> ExtractOutcomeStateTableHandle<'_>;
}

impl ExtractOutcomeStateTableAccess for super::RemoteTables {
    fn extract_outcome_state(&self) -> ExtractOutcomeStateTableHandle<'_> {
        ExtractOutcomeStateTableHandle {
            imp: self
                .imp
                .get_table::<ExtractOutcomeState>("extract_outcome_state"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct ExtractOutcomeStateInsertCallbackId(__sdk::CallbackId);
pub struct ExtractOutcomeStateDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for ExtractOutcomeStateTableHandle<'ctx> {
    type Row = ExtractOutcomeState;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = ExtractOutcomeState> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = ExtractOutcomeStateInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> ExtractOutcomeStateInsertCallbackId {
        ExtractOutcomeStateInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: ExtractOutcomeStateInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = ExtractOutcomeStateDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> ExtractOutcomeStateDeleteCallbackId {
        ExtractOutcomeStateDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: ExtractOutcomeStateDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<ExtractOutcomeState>("extract_outcome_state");
    _table.add_unique_constraint::<u64>("entity_id", |row| &row.entity_id);
}
pub struct ExtractOutcomeStateUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for ExtractOutcomeStateTableHandle<'ctx> {
    type UpdateCallbackId = ExtractOutcomeStateUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> ExtractOutcomeStateUpdateCallbackId {
        ExtractOutcomeStateUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: ExtractOutcomeStateUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<ExtractOutcomeState>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<ExtractOutcomeState>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `entity_id` unique index on the table `extract_outcome_state`,
/// which allows point queries on the field of the same name
/// via the [`ExtractOutcomeStateEntityIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.extract_outcome_state().entity_id().find(...)`.
pub struct ExtractOutcomeStateEntityIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<ExtractOutcomeState, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> ExtractOutcomeStateTableHandle<'ctx> {
    /// Get a handle on the `entity_id` unique index on the table `extract_outcome_state`.
    pub fn entity_id(&self) -> ExtractOutcomeStateEntityIdUnique<'ctx> {
        ExtractOutcomeStateEntityIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("entity_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> ExtractOutcomeStateEntityIdUnique<'ctx> {
    /// Find the subscribed row whose `entity_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<ExtractOutcomeState> {
        self.imp.find(col_val)
    }
}
