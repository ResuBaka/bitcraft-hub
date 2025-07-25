// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::capped_level_requirement_type::CappedLevelRequirement;
use super::experience_stack_f_32_type::ExperienceStackF32;
use super::item_stack_type::ItemStack;
use super::traveler_task_desc_type::TravelerTaskDesc;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `traveler_task_desc`.
///
/// Obtain a handle from the [`TravelerTaskDescTableAccess::traveler_task_desc`] method on [`super::RemoteTables`],
/// like `ctx.db.traveler_task_desc()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.traveler_task_desc().on_insert(...)`.
pub struct TravelerTaskDescTableHandle<'ctx> {
    imp: __sdk::TableHandle<TravelerTaskDesc>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `traveler_task_desc`.
///
/// Implemented for [`super::RemoteTables`].
pub trait TravelerTaskDescTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`TravelerTaskDescTableHandle`], which mediates access to the table `traveler_task_desc`.
    fn traveler_task_desc(&self) -> TravelerTaskDescTableHandle<'_>;
}

impl TravelerTaskDescTableAccess for super::RemoteTables {
    fn traveler_task_desc(&self) -> TravelerTaskDescTableHandle<'_> {
        TravelerTaskDescTableHandle {
            imp: self.imp.get_table::<TravelerTaskDesc>("traveler_task_desc"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct TravelerTaskDescInsertCallbackId(__sdk::CallbackId);
pub struct TravelerTaskDescDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for TravelerTaskDescTableHandle<'ctx> {
    type Row = TravelerTaskDesc;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = TravelerTaskDesc> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = TravelerTaskDescInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> TravelerTaskDescInsertCallbackId {
        TravelerTaskDescInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: TravelerTaskDescInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = TravelerTaskDescDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> TravelerTaskDescDeleteCallbackId {
        TravelerTaskDescDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: TravelerTaskDescDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<TravelerTaskDesc>("traveler_task_desc");
    _table.add_unique_constraint::<i32>("id", |row| &row.id);
}
pub struct TravelerTaskDescUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for TravelerTaskDescTableHandle<'ctx> {
    type UpdateCallbackId = TravelerTaskDescUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> TravelerTaskDescUpdateCallbackId {
        TravelerTaskDescUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: TravelerTaskDescUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<TravelerTaskDesc>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<TravelerTaskDesc>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `id` unique index on the table `traveler_task_desc`,
/// which allows point queries on the field of the same name
/// via the [`TravelerTaskDescIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.traveler_task_desc().id().find(...)`.
pub struct TravelerTaskDescIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<TravelerTaskDesc, i32>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> TravelerTaskDescTableHandle<'ctx> {
    /// Get a handle on the `id` unique index on the table `traveler_task_desc`.
    pub fn id(&self) -> TravelerTaskDescIdUnique<'ctx> {
        TravelerTaskDescIdUnique {
            imp: self.imp.get_unique_constraint::<i32>("id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> TravelerTaskDescIdUnique<'ctx> {
    /// Find the subscribed row whose `id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &i32) -> Option<TravelerTaskDesc> {
        self.imp.find(col_val)
    }
}
