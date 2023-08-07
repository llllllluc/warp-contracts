use controller::account::{AssetInfo, Fund};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, CosmosMsg, Uint64};

#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub warp_addr: Addr,
}

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub funds: Option<Vec<Fund>>,
    pub job_id: Option<Uint64>,
    pub is_job_account: bool,
    pub should_update_var_account_address: bool,
    pub msgs: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Generic(GenericMsg),
    WithdrawAssets(WithdrawAssetsMsg),
}

#[cw_serde]
pub struct GenericMsg {
    pub job_id: Option<Uint64>,
    pub msgs: Vec<CosmosMsg>,
}

#[cw_serde]
pub struct WithdrawAssetsMsg {
    pub asset_infos: Vec<AssetInfo>,
}

#[cw_serde]
pub struct ExecuteWasmMsg {}

#[cw_serde]
pub enum QueryMsg {}

#[cw_serde]
pub struct MigrateMsg {}
