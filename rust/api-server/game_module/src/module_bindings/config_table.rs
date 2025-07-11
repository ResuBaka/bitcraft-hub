// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::config_type::Config;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `config`.
///
/// Obtain a handle from the [`ConfigTableAccess::config`] method on [`super::RemoteTables`],
/// like `ctx.db.config()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.config().on_insert(...)`.
pub struct ConfigTableHandle<'ctx> {
    imp: __sdk::TableHandle<Config>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `config`.
///
/// Implemented for [`super::RemoteTables`].
pub trait ConfigTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`ConfigTableHandle`], which mediates access to the table `config`.
    fn config(&self) -> ConfigTableHandle<'_>;
}

impl ConfigTableAccess for super::RemoteTables {
    fn config(&self) -> ConfigTableHandle<'_> {
        ConfigTableHandle {
            imp: self.imp.get_table::<Config>("config"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct ConfigInsertCallbackId(__sdk::CallbackId);
pub struct ConfigDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for ConfigTableHandle<'ctx> {
    type Row = Config;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = Config> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = ConfigInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> ConfigInsertCallbackId {
        ConfigInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: ConfigInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = ConfigDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> ConfigDeleteCallbackId {
        ConfigDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: ConfigDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<Config>("config");
    _table.add_unique_constraint::<i32>("version", |row| &row.version);
}
pub struct ConfigUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for ConfigTableHandle<'ctx> {
    type UpdateCallbackId = ConfigUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> ConfigUpdateCallbackId {
        ConfigUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: ConfigUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<Config>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<Config>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `version` unique index on the table `config`,
/// which allows point queries on the field of the same name
/// via the [`ConfigVersionUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.config().version().find(...)`.
pub struct ConfigVersionUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<Config, i32>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> ConfigTableHandle<'ctx> {
    /// Get a handle on the `version` unique index on the table `config`.
    pub fn version(&self) -> ConfigVersionUnique<'ctx> {
        ConfigVersionUnique {
            imp: self.imp.get_unique_constraint::<i32>("version"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> ConfigVersionUnique<'ctx> {
    /// Find the subscribed row whose `version` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &i32) -> Option<Config> {
        self.imp.find(col_val)
    }
}
