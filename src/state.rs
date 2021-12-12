use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ContractError;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct BeverageStat {
    pub price: Uint128,
    pub amount: u8,
}

impl BeverageStat {
    pub fn refill(mut self, amount: u8) -> Result<Self, ContractError> {
        if self.amount + amount > 50 {
            Err(ContractError::BeverageNumberExceed {})
        } else {
            self.amount += amount;
            Ok(self)
        }
    }

    pub fn sell(mut self) -> Result<Self, ContractError> {
        if let Some(amount) = self.amount.checked_sub(1) {
            self.amount = amount;
            Ok(self)
        } else {
            Err(ContractError::BeverageIsOver {})
        }
    }
}

pub const BEVERAGES: Map<&str, BeverageStat> = Map::new("beverages");
pub const BALANCES: Map<&Addr, Uint128> = Map::new("balance");
pub const ADMIN: Item<Addr> = Item::new("admin");
