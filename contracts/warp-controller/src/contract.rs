use crate::error::map_contract_error;
use crate::state::{ACCOUNTS, CONFIG, FINISHED_JOBS, PENDING_JOBS};
use crate::{execute, query, state::STATE, ContractError};
use account::{GenericMsg, WithdrawAssetsMsg};
use controller::account::{
    Account, AssetInfo, Fund, FundTransferMsgs, TransferFromMsg, TransferNftMsg,
};
use controller::job::{Job, JobStatus};
use cosmwasm_schema::cw_serde;

use controller::{Config, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, State};
use cosmwasm_std::{
    entry_point, to_binary, Addr, Attribute, BalanceResponse, BankMsg, BankQuery, Binary, Coin,
    CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order, QueryRequest, Reply, Response, StdError,
    StdResult, SubMsgResult, Uint128, Uint64, WasmMsg,
};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, MultiIndex, UniqueIndex};
use resolver::condition::Condition;
use resolver::variable::{
    ExternalExpr, ExternalVariable, QueryExpr, QueryVariable, StaticVariable, UpdateFn, Variable,
    VariableKind,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        current_job_id: Uint64::one(),
        q: Uint64::zero(),
    };

    let config = Config {
        owner: deps
            .api
            .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?,
        fee_denom: msg.fee_denom,
        fee_collector: deps
            .api
            .addr_validate(&msg.fee_collector.unwrap_or_else(|| info.sender.to_string()))?,
        warp_account_code_id: msg.warp_account_code_id,
        minimum_reward: msg.minimum_reward,
        creation_fee_percentage: msg.creation_fee,
        cancellation_fee_percentage: msg.cancellation_fee,
        resolver_address: deps.api.addr_validate(&msg.resolver_address)?,
        t_max: msg.t_max,
        t_min: msg.t_min,
        a_max: msg.a_max,
        a_min: msg.a_min,
        q_max: msg.q_max,
    };

    if config.a_max < config.a_min {
        return Err(ContractError::MaxFeeUnderMinFee {});
    }

    if config.t_max < config.t_min {
        return Err(ContractError::MaxTimeUnderMinTime {});
    }

    if config.minimum_reward < config.a_min {
        return Err(ContractError::RewardSmallerThanFee {});
    }

    if config.creation_fee_percentage.u64() > 100 {
        return Err(ContractError::CreationFeeTooHigh {});
    }

    if config.cancellation_fee_percentage.u64() > 100 {
        return Err(ContractError::CancellationFeeTooHigh {});
    }

    STATE.save(deps.storage, &state)?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateJob(data) => execute::job::create_job(deps, env, info, data),
        ExecuteMsg::DeleteJob(data) => execute::job::delete_job(deps, env, info, data),
        ExecuteMsg::UpdateJob(data) => execute::job::update_job(deps, env, info, data),
        ExecuteMsg::ExecuteJob(data) => execute::job::execute_job(deps, env, info, data),
        ExecuteMsg::EvictJob(data) => execute::job::evict_job(deps, env, info, data),

        ExecuteMsg::CreateAccount(data) => execute::account::create_account(deps, env, info, data),

        ExecuteMsg::UpdateConfig(data) => execute::controller::update_config(deps, env, info, data),

        ExecuteMsg::MigrateAccounts(data) => {
            execute::controller::migrate_accounts(deps, env, info, data)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryJob(data) => to_binary(&query::job::query_job(deps, env, data)?),
        QueryMsg::QueryJobs(data) => to_binary(&query::job::query_jobs(deps, env, data)?),

        QueryMsg::QueryAccount(data) => to_binary(&query::account::query_account(deps, env, data)?),
        QueryMsg::QueryAccounts(data) => {
            to_binary(&query::account::query_accounts(deps, env, data)?)
        }

        QueryMsg::QueryConfig(data) => {
            to_binary(&query::controller::query_config(deps, env, data)?)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    //STATE
    #[cw_serde]
    pub struct V1State {
        pub current_job_id: Uint64,
        pub current_template_id: Uint64,
        pub q: Uint64,
    }

    const V1STATE: Item<V1State> = Item::new("state");
    let v1_state = V1STATE.load(deps.storage)?;

    STATE.save(
        deps.storage,
        &State {
            current_job_id: v1_state.current_job_id,
            q: v1_state.q,
        },
    )?;

    //CONFIG
    #[cw_serde]
    pub struct V1Config {
        pub owner: Addr,
        pub fee_denom: String,
        pub fee_collector: Addr,
        pub warp_account_code_id: Uint64,
        pub minimum_reward: Uint128,
        pub creation_fee_percentage: Uint64,
        pub cancellation_fee_percentage: Uint64,
        // maximum time for evictions
        pub t_max: Uint64,
        // minimum time for evictions
        pub t_min: Uint64,
        // maximum fee for evictions
        pub a_max: Uint128,
        // minimum fee for evictions
        pub a_min: Uint128,
        // maximum length of queue modifier for evictions
        pub q_max: Uint64,
    }

    const V1CONFIG: Item<V1Config> = Item::new("config");

    let v1_config = V1CONFIG.load(deps.storage)?;

    CONFIG.save(
        deps.storage,
        &Config {
            owner: v1_config.owner,
            fee_denom: v1_config.fee_denom,
            fee_collector: v1_config.fee_collector,
            warp_account_code_id: msg.warp_account_code_id,
            minimum_reward: v1_config.minimum_reward,
            creation_fee_percentage: v1_config.creation_fee_percentage,
            cancellation_fee_percentage: v1_config.cancellation_fee_percentage,
            resolver_address: deps.api.addr_validate(&msg.resolver_address)?,
            t_max: v1_config.t_max,
            t_min: v1_config.t_min,
            a_max: v1_config.a_max,
            a_min: v1_config.a_min,
            q_max: v1_config.q_max,
        },
    )?;

    //JOBS
    #[cw_serde]
    pub struct V1Job {
        pub id: Uint64,
        pub owner: Addr,
        pub last_update_time: Uint64,
        pub name: String,
        pub description: String,
        pub labels: Vec<String>,
        pub status: JobStatus,
        pub condition: Condition,
        pub msgs: Vec<String>,
        pub vars: Vec<V1Variable>,
        pub recurring: bool,
        pub requeue_on_evict: bool,
        pub reward: Uint128,
        pub assets_to_withdraw: Vec<AssetInfo>,
    }

    #[cw_serde]
    pub enum V1Variable {
        Static(V1StaticVariable),
        External(V1ExternalVariable),
        Query(V1QueryVariable),
    }

    #[cw_serde]
    pub struct V1StaticVariable {
        pub kind: VariableKind,
        pub name: String,
        pub value: String,
        pub update_fn: Option<UpdateFn>,
    }

    #[cw_serde]
    pub struct V1ExternalVariable {
        pub kind: VariableKind,
        pub name: String,
        pub init_fn: ExternalExpr,
        pub reinitialize: bool,
        pub value: Option<String>, //none if uninitialized
        pub update_fn: Option<UpdateFn>,
    }

    #[cw_serde]
    pub struct V1QueryVariable {
        pub kind: VariableKind,
        pub name: String,
        pub init_fn: QueryExpr,
        pub reinitialize: bool,
        pub value: Option<String>, //none if uninitialized
        pub update_fn: Option<UpdateFn>,
    }

    pub struct V1JobIndexes<'a> {
        pub reward: UniqueIndex<'a, (u128, u64), V1Job>,
        pub publish_time: MultiIndex<'a, u64, V1Job, u64>,
    }

    impl IndexList<V1Job> for V1JobIndexes<'_> {
        fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<V1Job>> + '_> {
            let v: Vec<&dyn Index<V1Job>> = vec![&self.reward, &self.publish_time];
            Box::new(v.into_iter())
        }
    }

    #[allow(non_snake_case)]
    pub fn V1_PENDING_JOBS<'a>() -> IndexedMap<'a, u64, V1Job, V1JobIndexes<'a>> {
        let indexes = V1JobIndexes {
            reward: UniqueIndex::new(
                |job| (job.reward.u128(), job.id.u64()),
                "pending_jobs__reward_v2",
            ),
            publish_time: MultiIndex::new(
                |_pk, job| job.last_update_time.u64(),
                "pending_jobs_v2",
                "pending_jobs__publish_timestamp_v2",
            ),
        };
        IndexedMap::new("pending_jobs_v2", indexes)
    }

    let job_keys: Result<Vec<_>, _> = V1_PENDING_JOBS()
        .keys(deps.storage, None, None, Order::Ascending)
        .collect();
    let job_keys = job_keys?;
    for job_key in job_keys {
        let v1_job = V1_PENDING_JOBS().load(deps.storage, job_key)?;
        let mut new_vars = vec![];
        for var in v1_job.vars {
            new_vars.push(match var {
                V1Variable::Static(v) => Variable::Static(StaticVariable {
                    kind: v.kind,
                    name: v.name,
                    encode: false,
                    value: v.value,
                    update_fn: v.update_fn,
                }),
                V1Variable::External(v) => Variable::External(ExternalVariable {
                    kind: v.kind,
                    name: v.name,
                    encode: false,
                    init_fn: v.init_fn,
                    reinitialize: v.reinitialize,
                    value: v.value,
                    update_fn: v.update_fn,
                }),
                V1Variable::Query(v) => Variable::Query(QueryVariable {
                    kind: v.kind,
                    name: v.name,
                    encode: false,
                    init_fn: v.init_fn,
                    reinitialize: v.reinitialize,
                    value: v.value,
                    update_fn: v.update_fn,
                }),
            })
        }

        let mut new_msgs = "[".to_string();

        for msg in v1_job.msgs {
            new_msgs.push_str(msg.as_str());
        }

        new_msgs.push(']');

        PENDING_JOBS().save(
            deps.storage,
            job_key,
            &Job {
                id: v1_job.id,
                owner: v1_job.owner,
                last_update_time: v1_job.last_update_time,
                name: v1_job.name,
                description: v1_job.description,
                labels: v1_job.labels,
                status: v1_job.status,
                condition: serde_json_wasm::to_string(&v1_job.condition)?,
                terminate_condition: None,
                msgs: new_msgs.to_string(),
                vars: serde_json_wasm::to_string(&new_vars)?,
                recurring: v1_job.recurring,
                requeue_on_evict: v1_job.requeue_on_evict,
                reward: v1_job.reward,
                assets_to_withdraw: v1_job.assets_to_withdraw,
            },
        )?;
    }

    #[allow(non_snake_case)]
    pub fn V1_FINISHED_JOBS<'a>() -> IndexedMap<'a, u64, V1Job, V1JobIndexes<'a>> {
        let indexes = V1JobIndexes {
            reward: UniqueIndex::new(
                |job| (job.reward.u128(), job.id.u64()),
                "finished_jobs__reward_v2",
            ),
            publish_time: MultiIndex::new(
                |_pk, job| job.last_update_time.u64(),
                "finished_jobs_v2",
                "finished_jobs__publish_timestamp_v2",
            ),
        };
        IndexedMap::new("finished_jobs_v2", indexes)
    }

    let job_keys: Result<Vec<_>, _> = V1_FINISHED_JOBS()
        .keys(deps.storage, None, None, Order::Ascending)
        .collect();
    let job_keys = job_keys?;
    for job_key in job_keys {
        let v1_job = V1_FINISHED_JOBS().load(deps.storage, job_key)?;
        let mut new_vars = vec![];
        for var in v1_job.vars {
            new_vars.push(match var {
                V1Variable::Static(v) => Variable::Static(StaticVariable {
                    kind: v.kind,
                    name: v.name,
                    encode: false,
                    value: v.value,
                    update_fn: v.update_fn,
                }),
                V1Variable::External(v) => Variable::External(ExternalVariable {
                    kind: v.kind,
                    name: v.name,
                    encode: false,
                    init_fn: v.init_fn,
                    reinitialize: v.reinitialize,
                    value: v.value,
                    update_fn: v.update_fn,
                }),
                V1Variable::Query(v) => Variable::Query(QueryVariable {
                    kind: v.kind,
                    name: v.name,
                    encode: false,
                    init_fn: v.init_fn,
                    reinitialize: v.reinitialize,
                    value: v.value,
                    update_fn: v.update_fn,
                }),
            })
        }

        let mut new_msgs = "[".to_string();

        for msg in v1_job.msgs {
            new_msgs.push_str(msg.as_str());
        }

        new_msgs.push(']');

        FINISHED_JOBS().save(
            deps.storage,
            job_key,
            &Job {
                id: v1_job.id,
                owner: v1_job.owner,
                last_update_time: v1_job.last_update_time,
                name: v1_job.name,
                description: v1_job.description,
                labels: v1_job.labels,
                status: v1_job.status,
                condition: serde_json_wasm::to_string(&v1_job.condition)?,
                terminate_condition: None,
                msgs: new_msgs,
                vars: serde_json_wasm::to_string(&new_vars)?,
                recurring: v1_job.recurring,
                requeue_on_evict: v1_job.requeue_on_evict,
                reward: v1_job.reward,
                assets_to_withdraw: v1_job.assets_to_withdraw,
            },
        )?;
    }

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        //account creation
        0 => {
            let reply = msg.result.into_result().map_err(StdError::generic_err)?;

            let event = reply
                .events
                .iter()
                .find(|event| {
                    event
                        .attributes
                        .iter()
                        .any(|attr| attr.key == "action" && attr.value == "instantiate")
                })
                .ok_or_else(|| StdError::generic_err("cannot find `instantiate` event"))?;

            let owner = event
                .attributes
                .iter()
                .cloned()
                .find(|attr| attr.key == "owner")
                .ok_or_else(|| StdError::generic_err("cannot find `owner` attribute"))?
                .value;

            let address = event
                .attributes
                .iter()
                .cloned()
                .find(|attr| attr.key == "contract_addr")
                .ok_or_else(|| StdError::generic_err("cannot find `contract_addr` attribute"))?
                .value;

            let funds: Vec<Coin> = serde_json_wasm::from_str(
                &event
                    .attributes
                    .iter()
                    .cloned()
                    .find(|attr| attr.key == "funds")
                    .ok_or_else(|| StdError::generic_err("cannot find `funds` attribute"))?
                    .value,
            )?;

            let cw_funds: Option<Vec<Fund>> = serde_json_wasm::from_str(
                &event
                    .attributes
                    .iter()
                    .cloned()
                    .find(|attr| attr.key == "cw_funds")
                    .ok_or_else(|| StdError::generic_err("cannot find `cw_funds` attribute"))?
                    .value,
            )?;

            let cw_funds_vec = match cw_funds {
                None => {
                    vec![]
                }
                Some(funds) => funds,
            };

            let mut msgs_vec: Vec<CosmosMsg> = vec![];

            for cw_fund in &cw_funds_vec {
                msgs_vec.push(CosmosMsg::Wasm(match cw_fund {
                    Fund::Cw20(cw20_fund) => WasmMsg::Execute {
                        contract_addr: deps
                            .api
                            .addr_validate(&cw20_fund.contract_addr)?
                            .to_string(),
                        msg: to_binary(&FundTransferMsgs::TransferFrom(TransferFromMsg {
                            owner: owner.clone(),
                            recipient: address.clone(),
                            amount: cw20_fund.amount,
                        }))?,
                        funds: vec![],
                    },
                    Fund::Cw721(cw721_fund) => WasmMsg::Execute {
                        contract_addr: deps
                            .api
                            .addr_validate(&cw721_fund.contract_addr)?
                            .to_string(),
                        msg: to_binary(&FundTransferMsgs::TransferNft(TransferNftMsg {
                            recipient: address.clone(),
                            token_id: cw721_fund.token_id.clone(),
                        }))?,
                        funds: vec![],
                    },
                }))
            }

            if ACCOUNTS().has(deps.storage, deps.api.addr_validate(&owner)?) {
                return Err(ContractError::AccountAlreadyExists {});
            }

            ACCOUNTS().save(
                deps.storage,
                deps.api.addr_validate(&owner)?,
                &Account {
                    owner: deps.api.addr_validate(&owner.clone())?,
                    account: deps.api.addr_validate(&address)?,
                },
            )?;
            Ok(Response::new()
                .add_attribute("action", "save_account")
                .add_attribute("owner", owner)
                .add_attribute("account_address", address)
                .add_attribute("funds", serde_json_wasm::to_string(&funds)?)
                .add_attribute("cw_funds", serde_json_wasm::to_string(&cw_funds_vec)?)
                .add_messages(msgs_vec))
        }
        //job execution
        _ => {
            let mut state = STATE.load(deps.storage)?;

            let new_status = match msg.result {
                SubMsgResult::Ok(_) => JobStatus::Executed,
                SubMsgResult::Err(_) => JobStatus::Failed,
            };

            let job = PENDING_JOBS().load(deps.storage, msg.id)?;
            PENDING_JOBS().remove(deps.storage, msg.id)?;

            let finished_job = FINISHED_JOBS().update(deps.storage, msg.id, |j| match j {
                None => Ok(Job {
                    id: job.id,
                    owner: job.owner,
                    last_update_time: job.last_update_time,
                    name: job.name,
                    description: job.description,
                    labels: job.labels,
                    status: new_status,
                    condition: job.condition,
                    terminate_condition: job.terminate_condition,
                    msgs: job.msgs,
                    vars: job.vars,
                    recurring: job.recurring,
                    requeue_on_evict: job.requeue_on_evict,
                    reward: job.reward,
                    assets_to_withdraw: job.assets_to_withdraw,
                }),
                Some(_) => Err(ContractError::JobAlreadyFinished {}),
            })?;

            let res_attrs = match msg.result {
                SubMsgResult::Err(e) => vec![Attribute::new(
                    "transaction_error",
                    format!("{}. {}", &e, map_contract_error(&e)),
                )],
                _ => vec![],
            };

            let mut msgs = vec![];
            let mut new_job_attrs = vec![];

            let account = ACCOUNTS().load(deps.storage, finished_job.owner.clone())?;
            let config = CONFIG.load(deps.storage)?;

            //assume reward.amount == warp token allowance
            let fee = finished_job.reward * Uint128::from(config.creation_fee_percentage)
                / Uint128::new(100);

            let account_amount = deps
                .querier
                .query::<BalanceResponse>(&QueryRequest::Bank(BankQuery::Balance {
                    address: account.account.to_string(),
                    denom: config.fee_denom.clone(),
                }))?
                .amount
                .amount;

            if finished_job.recurring {
                if account_amount < fee + finished_job.reward {
                    new_job_attrs.push(Attribute::new("action", "recur_job"));
                    new_job_attrs.push(Attribute::new("creation_status", "failed_insufficient_fee"))
                } else if !(finished_job.status == JobStatus::Executed
                    || finished_job.status == JobStatus::Failed)
                {
                    new_job_attrs.push(Attribute::new("action", "recur_job"));
                    new_job_attrs.push(Attribute::new(
                        "creation_status",
                        "failed_invalid_job_status",
                    ));
                } else {
                    let new_vars: String = deps.querier.query_wasm_smart(
                        config.resolver_address.clone(),
                        &resolver::QueryMsg::QueryApplyVarFn(resolver::QueryApplyVarFnMsg {
                            vars: finished_job.vars,
                            status: finished_job.status.clone(),
                        }),
                    )?;

                    let should_terminate_job: bool;
                    match finished_job.terminate_condition.clone() {
                        Some(terminate_condition) => {
                            let resolution: StdResult<bool> = deps.querier.query_wasm_smart(
                                config.resolver_address,
                                &resolver::QueryMsg::QueryResolveCondition(
                                    resolver::QueryResolveConditionMsg {
                                        condition: terminate_condition,
                                        vars: new_vars.clone(),
                                    },
                                ),
                            );
                            if let Err(e) = resolution {
                                should_terminate_job = true;
                                new_job_attrs.push(Attribute::new("action", "recur_job"));
                                new_job_attrs.push(Attribute::new(
                                    "job_terminate_condition_status",
                                    "invalid",
                                ));
                                new_job_attrs.push(Attribute::new(
                                    "creation_status",
                                    format!(
                                        "terminated_due_to_terminate_condition_resolves_to_error. {}",
                                        e
                                    ),
                                ));
                            } else {
                                new_job_attrs.push(Attribute::new(
                                    "job_terminate_condition_status",
                                    "valid",
                                ));
                                if resolution? {
                                    should_terminate_job = true;
                                    new_job_attrs.push(Attribute::new("action", "recur_job"));
                                    new_job_attrs.push(Attribute::new(
                                        "creation_status",
                                        "terminated_due_to_terminate_condition_resolves_to_true",
                                    ));
                                } else {
                                    should_terminate_job = false;
                                }
                            }
                        }
                        None => {
                            should_terminate_job = false;
                        }
                    }

                    if !should_terminate_job {
                        let new_job = PENDING_JOBS().update(
                            deps.storage,
                            state.current_job_id.u64(),
                            |s| match s {
                                None => Ok(Job {
                                    id: state.current_job_id,
                                    owner: finished_job.owner.clone(),
                                    last_update_time: Uint64::from(env.block.time.seconds()),
                                    name: finished_job.name.clone(),
                                    description: finished_job.description,
                                    labels: finished_job.labels,
                                    status: JobStatus::Pending,
                                    condition: finished_job.condition.clone(),
                                    terminate_condition: finished_job.terminate_condition.clone(),
                                    vars: new_vars,
                                    requeue_on_evict: finished_job.requeue_on_evict,
                                    recurring: finished_job.recurring,
                                    msgs: finished_job.msgs.clone(),
                                    reward: finished_job.reward,
                                    assets_to_withdraw: finished_job.assets_to_withdraw,
                                }),
                                Some(_) => Err(ContractError::JobAlreadyExists {}),
                            },
                        )?;

                        state.current_job_id = state.current_job_id.checked_add(Uint64::new(1))?;
                        state.q = state.q.checked_add(Uint64::new(1))?;

                        msgs.push(
                            //send reward to controller
                            WasmMsg::Execute {
                                contract_addr: account.account.to_string(),
                                msg: to_binary(&account::ExecuteMsg::Generic(GenericMsg {
                                    msgs: vec![CosmosMsg::Bank(BankMsg::Send {
                                        to_address: config.fee_collector.to_string(),
                                        amount: vec![Coin::new(
                                            (fee).u128(),
                                            config.fee_denom.clone(),
                                        )],
                                    })],
                                }))?,
                                funds: vec![],
                            },
                        );

                        msgs.push(
                            //send reward to controller
                            WasmMsg::Execute {
                                contract_addr: account.account.to_string(),
                                msg: to_binary(&account::ExecuteMsg::Generic(GenericMsg {
                                    msgs: vec![CosmosMsg::Bank(BankMsg::Send {
                                        to_address: env.contract.address.to_string(),
                                        amount: vec![Coin::new(
                                            (new_job.reward).u128(),
                                            config.fee_denom,
                                        )],
                                    })],
                                }))?,
                                funds: vec![],
                            },
                        );

                        msgs.push(
                            //withdraw all assets that are listed
                            WasmMsg::Execute {
                                contract_addr: account.account.to_string(),
                                msg: to_binary(&account::ExecuteMsg::WithdrawAssets(
                                    WithdrawAssetsMsg {
                                        asset_infos: new_job.assets_to_withdraw,
                                    },
                                ))?,
                                funds: vec![],
                            },
                        );

                        new_job_attrs.push(Attribute::new("action", "create_job"));
                        new_job_attrs.push(Attribute::new("job_id", new_job.id));
                        new_job_attrs.push(Attribute::new("job_owner", new_job.owner));
                        new_job_attrs.push(Attribute::new("job_name", new_job.name));
                        new_job_attrs.push(Attribute::new(
                            "job_status",
                            serde_json_wasm::to_string(&new_job.status)?,
                        ));
                        new_job_attrs.push(Attribute::new(
                            "job_condition",
                            serde_json_wasm::to_string(&new_job.condition)?,
                        ));
                        new_job_attrs.push(Attribute::new(
                            "job_msgs",
                            serde_json_wasm::to_string(&new_job.msgs)?,
                        ));
                        new_job_attrs.push(Attribute::new("job_reward", new_job.reward));
                        new_job_attrs.push(Attribute::new("job_creation_fee", fee));
                        new_job_attrs.push(Attribute::new(
                            "job_last_updated_time",
                            new_job.last_update_time,
                        ));
                        new_job_attrs.push(Attribute::new("sub_action", "recur_job"));
                    }
                }
            }

            STATE.save(deps.storage, &state)?;

            Ok(Response::new()
                .add_attribute("action", "execute_reply")
                .add_attribute("job_id", job.id)
                .add_attributes(res_attrs)
                .add_attributes(new_job_attrs)
                .add_messages(msgs))
        }
    }
}
