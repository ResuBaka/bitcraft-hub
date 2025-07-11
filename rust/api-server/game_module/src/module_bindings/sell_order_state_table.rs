// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use super::auction_listing_state_type::AuctionListingState;
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

/// Table handle for the table `sell_order_state`.
///
/// Obtain a handle from the [`SellOrderStateTableAccess::sell_order_state`] method on [`super::RemoteTables`],
/// like `ctx.db.sell_order_state()`.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.sell_order_state().on_insert(...)`.
pub struct SellOrderStateTableHandle<'ctx> {
    imp: __sdk::TableHandle<AuctionListingState>,
    ctx: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

#[allow(non_camel_case_types)]
/// Extension trait for access to the table `sell_order_state`.
///
/// Implemented for [`super::RemoteTables`].
pub trait SellOrderStateTableAccess {
    #[allow(non_snake_case)]
    /// Obtain a [`SellOrderStateTableHandle`], which mediates access to the table `sell_order_state`.
    fn sell_order_state(&self) -> SellOrderStateTableHandle<'_>;
}

impl SellOrderStateTableAccess for super::RemoteTables {
    fn sell_order_state(&self) -> SellOrderStateTableHandle<'_> {
        SellOrderStateTableHandle {
            imp: self
                .imp
                .get_table::<AuctionListingState>("sell_order_state"),
            ctx: std::marker::PhantomData,
        }
    }
}

pub struct SellOrderStateInsertCallbackId(__sdk::CallbackId);
pub struct SellOrderStateDeleteCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::Table for SellOrderStateTableHandle<'ctx> {
    type Row = AuctionListingState;
    type EventContext = super::EventContext;

    fn count(&self) -> u64 {
        self.imp.count()
    }
    fn iter(&self) -> impl Iterator<Item = AuctionListingState> + '_ {
        self.imp.iter()
    }

    type InsertCallbackId = SellOrderStateInsertCallbackId;

    fn on_insert(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> SellOrderStateInsertCallbackId {
        SellOrderStateInsertCallbackId(self.imp.on_insert(Box::new(callback)))
    }

    fn remove_on_insert(&self, callback: SellOrderStateInsertCallbackId) {
        self.imp.remove_on_insert(callback.0)
    }

    type DeleteCallbackId = SellOrderStateDeleteCallbackId;

    fn on_delete(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row) + Send + 'static,
    ) -> SellOrderStateDeleteCallbackId {
        SellOrderStateDeleteCallbackId(self.imp.on_delete(Box::new(callback)))
    }

    fn remove_on_delete(&self, callback: SellOrderStateDeleteCallbackId) {
        self.imp.remove_on_delete(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn register_table(client_cache: &mut __sdk::ClientCache<super::RemoteModule>) {
    let _table = client_cache.get_or_make_table::<AuctionListingState>("sell_order_state");
    _table.add_unique_constraint::<u64>("entity_id", |row| &row.entity_id);
}
pub struct SellOrderStateUpdateCallbackId(__sdk::CallbackId);

impl<'ctx> __sdk::TableWithPrimaryKey for SellOrderStateTableHandle<'ctx> {
    type UpdateCallbackId = SellOrderStateUpdateCallbackId;

    fn on_update(
        &self,
        callback: impl FnMut(&Self::EventContext, &Self::Row, &Self::Row) + Send + 'static,
    ) -> SellOrderStateUpdateCallbackId {
        SellOrderStateUpdateCallbackId(self.imp.on_update(Box::new(callback)))
    }

    fn remove_on_update(&self, callback: SellOrderStateUpdateCallbackId) {
        self.imp.remove_on_update(callback.0)
    }
}

#[doc(hidden)]
pub(super) fn parse_table_update(
    raw_updates: __ws::TableUpdate<__ws::BsatnFormat>,
) -> __sdk::Result<__sdk::TableUpdate<AuctionListingState>> {
    __sdk::TableUpdate::parse_table_update(raw_updates).map_err(|e| {
        __sdk::InternalError::failed_parse("TableUpdate<AuctionListingState>", "TableUpdate")
            .with_cause(e)
            .into()
    })
}

/// Access to the `entity_id` unique index on the table `sell_order_state`,
/// which allows point queries on the field of the same name
/// via the [`SellOrderStateEntityIdUnique::find`] method.
///
/// Users are encouraged not to explicitly reference this type,
/// but to directly chain method calls,
/// like `ctx.db.sell_order_state().entity_id().find(...)`.
pub struct SellOrderStateEntityIdUnique<'ctx> {
    imp: __sdk::UniqueConstraintHandle<AuctionListingState, u64>,
    phantom: std::marker::PhantomData<&'ctx super::RemoteTables>,
}

impl<'ctx> SellOrderStateTableHandle<'ctx> {
    /// Get a handle on the `entity_id` unique index on the table `sell_order_state`.
    pub fn entity_id(&self) -> SellOrderStateEntityIdUnique<'ctx> {
        SellOrderStateEntityIdUnique {
            imp: self.imp.get_unique_constraint::<u64>("entity_id"),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'ctx> SellOrderStateEntityIdUnique<'ctx> {
    /// Find the subscribed row whose `entity_id` column value is equal to `col_val`,
    /// if such a row is present in the client cache.
    pub fn find(&self, col_val: &u64) -> Option<AuctionListingState> {
        self.imp.find(col_val)
    }
}
