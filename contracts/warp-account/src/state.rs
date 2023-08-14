use account::Config;
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");

// Key is the sub account address, value is the ID of the pending job currently using it
pub const IN_USE_SUB_ACCOUNTS: Map<String, u64> = Map::new("in_use_sub_accounts");

// Key is the sub account address, value is a dummy data to make it behave like a set
pub const FREE_SUB_ACCOUNTS: Map<String, u64> = Map::new("free_sub_accounts");
