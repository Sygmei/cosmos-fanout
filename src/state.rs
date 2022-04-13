use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Coin};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner: Addr,
    pub only_owner_can_register_beneficiary: bool,
}

pub const STATE: Item<State> = Item::new("state");
pub const BENEFICIARIES: Map<Addr, Vec<Coin>> = Map::new("beneficiaries");
pub const REMOVED_BENEFICIARIES: Map<Addr, Vec<Coin>> = Map::new("removed_beneficiaries");
pub const DONATORS: Map<Addr, Vec<Coin>> = Map::new("donators");
