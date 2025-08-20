use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
pub struct AuctionListingState {
    pub entity_id: u64,
    pub owner_entity_id: u64,
    pub claim_entity_id: u64,
    pub item_id: i32,
    pub item_type: i32,
    pub price_threshold: i32,
    pub quantity: i32,
    pub timestamp: crate::shared::timestamp::Timestamp,
    pub stored_coins: i32,
    pub region: i32,
}

pub struct AuctionListingStateBuilder {
    pub entity_id: u64,
    pub owner_entity_id: u64,
    pub claim_entity_id: u64,
    pub item_id: i32,
    pub item_type: i32,
    pub price_threshold: i32,
    pub quantity: i32,
    pub timestamp: crate::shared::timestamp::Timestamp,
    pub stored_coins: i32,
    pub region: i32,
}

impl AuctionListingStateBuilder {
    pub fn new(building_nickname_state: game_module::module_bindings::AuctionListingState) -> Self {
        Self {
            entity_id: building_nickname_state.entity_id,
            owner_entity_id: building_nickname_state.owner_entity_id,
            claim_entity_id: building_nickname_state.claim_entity_id,
            item_id: building_nickname_state.item_id,
            item_type: building_nickname_state.item_type,
            price_threshold: building_nickname_state.price_threshold,
            quantity: building_nickname_state.quantity,
            timestamp: building_nickname_state.timestamp.into(),
            stored_coins: building_nickname_state.stored_coins,
            region: 0,
        }
    }

    pub fn with_region(mut self, region: i32) -> Self {
        self.region = region;
        self
    }

    pub fn build(self) -> AuctionListingState {
        AuctionListingState {
            entity_id: self.entity_id,
            owner_entity_id: self.owner_entity_id,
            claim_entity_id: self.claim_entity_id,
            item_id: self.item_id,
            item_type: self.item_type,
            price_threshold: self.price_threshold,
            quantity: self.quantity,
            timestamp: self.timestamp,
            stored_coins: self.stored_coins,
            region: self.region,
        }
    }
}
