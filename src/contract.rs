#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, StdError, StdResult, Storage, Uint128,
};
use cw2::{get_contract_version, set_contract_version};
use cw_storage_plus::Map;
use semver::Version;

use crate::error::ContractError;
use crate::msg::{
    BeneficiaryListResponse, BeneficiaryResponse, DonatorListResponse, ExecuteMsg, InstantiateMsg,
    MigrateMsg, PotDonatorResponse, QueryMsg,
};
use crate::state::{State, BENEFICIARIES, DONATORS, REMOVED_BENEFICIARIES, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cosmos-fanout";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let state = State {
        owner: info.sender.clone(),
        only_owner_can_register_beneficiary: msg.only_owner_can_register_beneficiary,
    };
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version: Version = CONTRACT_VERSION.parse()?;
    let storage_version: Version = get_contract_version(deps.storage)?.version.parse()?;

    if storage_version < version {
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    }
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    // Acts like a message dispatcher
    // Will reroute the message to the correct handler
    match msg {
        ExecuteMsg::RegisterBeneficiaryAsOwner { beneficiary } => {
            register_beneficiary(deps, info, beneficiary)
        }
        ExecuteMsg::RegisterBeneficiary {} => {
            register_beneficiary(deps, info.clone(), info.sender.clone().to_string())
        }
        ExecuteMsg::RemoveBeneficiary {} => {
            remove_beneficiary(deps, info.clone(), info.sender.clone().to_string())
        }
        ExecuteMsg::RemoveBeneficiaryAsOwner { beneficiary } => {
            remove_beneficiary(deps, info.clone(), beneficiary)
        }
        ExecuteMsg::AddToPot {} => add_to_pot(deps, info),
    }
}

pub fn remove_beneficiary(
    deps: DepsMut,
    info: MessageInfo,
    beneficiary: String,
) -> Result<Response, ContractError> {
    let beneficiary_addr = deps.api.addr_validate(&beneficiary)?;
    let state = STATE.load(deps.storage).expect("unable to load state");
    if info.sender != beneficiary && state.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    if !BENEFICIARIES.has(deps.storage, beneficiary_addr.clone()) {
        return Err(ContractError::NotABeneficiary {});
    }
    if let Ok(beneficiairies_funds) = BENEFICIARIES.load(deps.storage, beneficiary_addr.clone()) {
        REMOVED_BENEFICIARIES.save(
            deps.storage,
            beneficiary_addr.clone(),
            &beneficiairies_funds,
        )?
    }
    BENEFICIARIES.remove(deps.storage, beneficiary_addr);
    Ok(Response::new().add_attribute("method", "remove_beneficiary"))
}

pub fn register_beneficiary(
    deps: DepsMut,
    info: MessageInfo,
    beneficiary: String,
) -> Result<Response, ContractError> {
    let beneficiary_addr = deps.api.addr_validate(&beneficiary)?;
    let state = STATE.load(deps.storage).expect("unable to load state");
    if state.only_owner_can_register_beneficiary && state.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    if BENEFICIARIES.has(deps.storage, beneficiary_addr.clone()) {
        return Err(ContractError::AlreadyABeneficiary {});
    }
    // Restore old donations, useful for keeping track of all donations made to a beneficiary
    let mut old_donations: Vec<Coin> = Vec::new();
    if REMOVED_BENEFICIARIES.has(deps.storage, beneficiary_addr.clone()) {
        old_donations = REMOVED_BENEFICIARIES.load(deps.storage, beneficiary_addr.clone())?;
    }
    let result = BENEFICIARIES.save(deps.storage, beneficiary_addr, &mut old_donations);
    if result.is_err() {
        return Err(ContractError::Unauthorized {});
    }
    Ok(Response::new().add_attribute("method", "register_beneficiary"))
}

fn split_coins_into_parts(coins: &Vec<Coin>, parts: u32) -> Vec<Vec<Coin>> {
    let mut split_coins: Vec<Vec<Coin>> = Vec::new();
    for _ in 0..parts {
        let mut coin_repartition = Vec::new();

        for coin in coins {
            let equal_amount_for_coin = coin.amount.checked_div(Uint128::from(parts)).unwrap();
            let new_split_coin = Coin {
                denom: coin.denom.clone(),
                amount: equal_amount_for_coin,
            };
            coin_repartition.push(new_split_coin);
        }

        split_coins.push(coin_repartition);
    }

    return split_coins;
}

fn register_donation_infos(
    storage: &mut dyn Storage,
    donator_addr: Addr,
    mut donation_funds: Vec<Coin>,
) {
    let _ = DONATORS.update(
        storage,
        donator_addr,
        |donator| -> Result<_, ContractError> {
            if let Some(mut donator_funds) = donator {
                donator_funds.append(&mut donation_funds);
                Ok(donator_funds)
            } else {
                Ok(donation_funds)
            }
        },
    );
}

fn register_beneficiary_donation_infos(
    storage: &mut dyn Storage,
    beneficiary_addr: Addr,
    mut donation_funds: Vec<Coin>,
) {
    let _ = BENEFICIARIES.update(
        storage,
        beneficiary_addr,
        |beneficiary_funds| -> Result<_, ContractError> {
            if let Some(mut beneficiary_funds) = beneficiary_funds {
                beneficiary_funds.append(&mut donation_funds);
                Ok(beneficiary_funds)
            } else {
                Ok(donation_funds)
            }
        },
    );
}

pub fn add_to_pot(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut amount_of_beneficiaries = 0;
    for _beneficiary in BENEFICIARIES.keys(deps.storage, None, None, cosmwasm_std::Order::Ascending)
    {
        amount_of_beneficiaries += 1;
    }
    if amount_of_beneficiaries < 1 {
        return Err(ContractError::NoBeneficiaries {});
    }
    register_donation_infos(deps.storage, info.sender.clone(), info.funds.clone());
    let funds_for_each = split_coins_into_parts(&info.funds, amount_of_beneficiaries);

    // Building a new Response
    let mut response: Response<Empty> = Response::new();

    // Distributing money amongst beneficiaries
    let mut beneficiaries_list: Vec<Addr> = Vec::new();
    for beneficiary in BENEFICIARIES.keys(deps.storage, None, None, cosmwasm_std::Order::Ascending)
    {
        beneficiaries_list.push(beneficiary?);
    }
    for (beneficiary, coin_part) in beneficiaries_list.iter().zip(funds_for_each) {
        // We are adding a new "BankMsg" for each beneficiary
        response = response.add_message(BankMsg::Send {
            amount: coins(coin_part[0].amount.u128(), coin_part[0].denom.clone()),
            to_address: beneficiary.clone().into_string(),
        });
        register_beneficiary_donation_infos(deps.storage, beneficiary.clone(), coin_part);
    }

    let beneficiaries_as_str = format!("{:?}", beneficiaries_list);
    response = response.add_attribute("beneficiaries", beneficiaries_as_str);
    response = response.add_attribute(
        "amount_of_beneficiaries",
        amount_of_beneficiaries.to_string(),
    );
    Ok(response.add_attribute("method", "add_to_pot"))
}

pub fn admin_action(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |state| -> Result<_, ContractError> {
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "admin_action"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetState {} => to_binary(&query_state(deps)?),
        QueryMsg::GetDonator { donator } => to_binary(&query_donator(deps, donator)?),
        QueryMsg::GetBeneficiary { beneficiary } => {
            to_binary(&query_beneficiary(deps, beneficiary, &BENEFICIARIES)?)
        }
        QueryMsg::GetRemovedBeneficiary { beneficiary } => to_binary(&query_beneficiary(
            deps,
            beneficiary,
            &REMOVED_BENEFICIARIES,
        )?),
        QueryMsg::GetAllDonators {} => to_binary(&query_all_donators(deps)?),
        QueryMsg::GetAllBeneficiaries {} => {
            to_binary(&query_all_beneficiaries(deps, &BENEFICIARIES)?)
        }
        QueryMsg::GetAllRemovedBeneficiaries {} => {
            to_binary(&query_all_beneficiaries(deps, &REMOVED_BENEFICIARIES)?)
        }
    }
}

fn query_state(deps: Deps) -> StdResult<State> {
    if let Ok(state) = STATE.load(deps.storage) {
        return Ok(State {
            owner: state.owner,
            only_owner_can_register_beneficiary: state.only_owner_can_register_beneficiary,
        });
    }
    Err(StdError::GenericErr {
        msg: "unable to load contract state".to_string(),
    })
}

fn query_donator(deps: Deps, donator: String) -> StdResult<PotDonatorResponse> {
    let donator_addr = deps.api.addr_validate(&donator)?;
    if let Ok(donator_infos) = DONATORS.load(deps.storage, donator_addr.clone()) {
        return Ok(PotDonatorResponse {
            donator: donator_addr,
            donations: donator_infos,
        });
    }

    return Err(StdError::GenericErr {
        msg: "Not a donator".to_string(),
    });
}

fn query_beneficiary(
    deps: Deps,
    beneficiary: String,
    target: &Map<Addr, Vec<Coin>>,
) -> StdResult<BeneficiaryResponse> {
    let beneficiary_addr = deps.api.addr_validate(&beneficiary)?;
    if let Ok(beneficiary_infos) = target.load(deps.storage, beneficiary_addr.clone()) {
        return Ok(BeneficiaryResponse {
            beneficiary: beneficiary_addr,
            received_donations: beneficiary_infos,
        });
    }

    return Err(StdError::GenericErr {
        msg: "Not a beneficiary".to_string(),
    });
}

fn query_all_donators(deps: Deps) -> StdResult<DonatorListResponse> {
    let donators = DONATORS.keys(deps.storage, None, None, cosmwasm_std::Order::Ascending);
    let donators: Result<Vec<Addr>, _> = donators.collect();
    return Ok(DonatorListResponse {
        donators: donators?,
    });
}

fn query_all_beneficiaries(
    deps: Deps,
    target: &Map<Addr, Vec<Coin>>,
) -> StdResult<BeneficiaryListResponse> {
    let beneficiaries = target.keys(deps.storage, None, None, cosmwasm_std::Order::Ascending);
    let beneficiaries: Result<Vec<Addr>, _> = beneficiaries.collect();
    return Ok(BeneficiaryListResponse {
        beneficiaries: beneficiaries?,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};
    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(0, "token"));
        let msg = InstantiateMsg {
            only_owner_can_register_beneficiary: false,
        };
        let owner_info = mock_info("owner", &coins(1000, "token"));
        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());
        // it worked, let's query the state
        let state = STATE.load(&deps.storage).expect("failed to load state");
        assert_eq!(state.owner, owner_info.sender, "invalid owner");
    }
    #[test]
    fn funds_distribution_2_beneficiary() {
        // Instantiating smart contract
        let mut deps = mock_dependencies_with_balance(&coins(0, "token"));
        let msg = InstantiateMsg {
            only_owner_can_register_beneficiary: false,
        };
        let owner_info = mock_info("owner", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), owner_info, msg).unwrap();

        // Create two beneficiaries
        let beneficiary1_info = mock_info("beneficiary1", &coins(1, "token"));
        let beneficiary2_info = mock_info("beneficiary2", &coins(1, "token"));

        // Register beneficiaries
        execute(
            deps.as_mut(),
            mock_env(),
            beneficiary1_info.clone(),
            ExecuteMsg::RegisterBeneficiary {},
        )
        .expect("error occured while beneficiary2 tried to register");
        execute(
            deps.as_mut(),
            mock_env(),
            beneficiary2_info.clone(),
            ExecuteMsg::RegisterBeneficiary {},
        )
        .expect("error occured while beneficiary2 tried to register");

        // Create one donator
        let donator1_info = mock_info("donator1", &coins(1000, "token"));

        // Donate 1000 tokens
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            donator1_info,
            ExecuteMsg::AddToPot {},
        )
        .expect("error occured while donating");

        // Query beneficiary donated funds
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary1_info.sender.to_string(),
            },
        )
        .expect("could not query beneficiary1 funds");
        let beneficiary1_funds: BeneficiaryResponse = from_binary(&res).unwrap();
        assert!(beneficiary1_funds.received_donations[0].amount == Uint128::from(500u32));

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary2_info.sender.to_string(),
            },
        )
        .expect("could not query beneficiary2 funds");
        let beneficiary2_funds: BeneficiaryResponse = from_binary(&res).unwrap();
        assert!(beneficiary2_funds.received_donations[0].amount == Uint128::from(500u32));
    }
    #[test]
    fn test_only_admin_can_add_beneficiaries() {
        // Instantiating smart contract
        let mut deps = mock_dependencies_with_balance(&coins(0, "token"));
        let msg = InstantiateMsg {
            only_owner_can_register_beneficiary: true,
        };
        let owner_info = mock_info("owner", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

        // Create one beneficiary
        let beneficiary_info = mock_info("beneficiary1", &coins(1, "token"));

        // Register beneficiaries as user (should be failing)
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            beneficiary_info.clone(),
            ExecuteMsg::RegisterBeneficiary {},
        )
        .expect_err("should be Unauthorized");

        // Register beneficiaries as owner (should be working)
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            owner_info.clone(),
            ExecuteMsg::RegisterBeneficiaryAsOwner {
                beneficiary: beneficiary_info.sender.clone().to_string(),
            },
        )
        .expect("owner failed to register beneficiary1 as a beneficiary");

        // Create one donator
        let donator1_info = mock_info("donator1", &coins(1000, "token"));

        // Donate 1000 tokens
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            donator1_info,
            ExecuteMsg::AddToPot {},
        )
        .expect("failed to donate tokens");

        // Query beneficiary donated funds
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary_info.sender.to_string(),
            },
        )
        .expect("could not query beneficiary1 funds");
        let beneficiary1_funds: BeneficiaryResponse = from_binary(&res).unwrap();
        assert!(beneficiary1_funds.received_donations[0].amount == Uint128::from(1000u32));
    }
    #[test]
    fn test_only_admin_or_beneficiary_can_remove_beneficiary() {
        // Instantiating smart contract
        let mut deps = mock_dependencies_with_balance(&coins(0, "token"));
        let msg = InstantiateMsg {
            only_owner_can_register_beneficiary: false,
        };
        let owner_info = mock_info("owner", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

        // Create two beneficiaries
        let beneficiary1_info = mock_info("beneficiary1", &coins(1, "token"));
        let beneficiary2_info = mock_info("beneficiary2", &coins(1, "token"));

        // Register beneficiaries as user
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            beneficiary1_info.clone(),
            ExecuteMsg::RegisterBeneficiary {},
        )
        .expect("failed to add beneficiary1 as beneficiary");
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            beneficiary2_info.clone(),
            ExecuteMsg::RegisterBeneficiary {},
        )
        .expect("failed to add beneficiary2 as beneficiary");

        // Check that both beneficiaries are actually registered
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary1_info.sender.to_string(),
            },
        )
        .expect("beneficiary1 should be a beneficiary");
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary2_info.sender.to_string(),
            },
        )
        .expect("beneficiary2 should be a beneficiary");

        // Non-owner should not be able to call "remove_beneficiary_as_owner"
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            beneficiary1_info.clone(),
            ExecuteMsg::RemoveBeneficiaryAsOwner {
                beneficiary: beneficiary2_info.sender.clone().to_string(),
            },
        )
        .expect_err("should be Unauthorized");

        // Non-owner should be able to remove itself
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            beneficiary1_info.clone(),
            ExecuteMsg::RemoveBeneficiary {},
        )
        .expect("beneficiary1 should be able to remove itself from beneficiaries");
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary1_info.sender.to_string(),
            },
        )
        .expect_err("beneficiary1 should not be a beneficiary anymore");
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetRemovedBeneficiary {
                beneficiary: beneficiary1_info.sender.to_string(),
            },
        )
        .expect("beneficiary1 should be in removed beneficiaries");

        // Owner should be able to remove anyone
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            owner_info.clone(),
            ExecuteMsg::RemoveBeneficiaryAsOwner {
                beneficiary: beneficiary2_info.sender.clone().to_string(),
            },
        )
        .expect("owner should be able to remove beneficiary2");
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary2_info.sender.to_string(),
            },
        )
        .expect_err("beneficiary2 should not be a beneficiary anymore");

        // Both beneficiaries should be in the removed beneficiaries list
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetAllRemovedBeneficiaries {},
        )
        .expect("failed fetching beneficiairy list");
        let removed_beneficiaries_list: BeneficiaryListResponse = from_binary(&res).unwrap();
        assert_eq!(removed_beneficiaries_list.beneficiaries.len(), 2);
        assert_eq!(
            removed_beneficiaries_list.beneficiaries[0],
            beneficiary1_info.sender
        );
        assert_eq!(
            removed_beneficiaries_list.beneficiaries[1],
            beneficiary2_info.sender
        );
    }
    #[test]
    fn test_split_between_100_beneficiaries() {
        // Instantiating smart contract
        let mut deps = mock_dependencies_with_balance(&coins(0, "token"));
        let msg = InstantiateMsg {
            only_owner_can_register_beneficiary: false,
        };
        let owner_info = mock_info("owner", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

        // Create 100 beneficiaries
        let mut beneficiaries_infos: Vec<MessageInfo> = Vec::new();
        for i in 1..100 {
            beneficiaries_infos.push(mock_info(&format!("beneficiary{}", i), &coins(1, "token")))
        }
        // Register each as a beneficiary
        for beneficiary_info in &beneficiaries_infos {
            execute(
                deps.as_mut(),
                mock_env(),
                beneficiary_info.clone(),
                ExecuteMsg::RegisterBeneficiary {},
            )
            .expect("error occured while beneficiary tried to register");
        }

        // Donate funds
        let donator_infos = mock_info("generous_donator", &coins(4500, "token"));
        execute(
            deps.as_mut(),
            mock_env(),
            donator_infos.clone(),
            ExecuteMsg::AddToPot {},
        )
        .expect("donation failed");

        // Each beneficiary should have 45 tokens
        for beneficiary_info in &beneficiaries_infos {
            let beneficiary_query_resp = query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::GetBeneficiary {
                    beneficiary: beneficiary_info.sender.to_string(),
                },
            )
            .expect("query funds of beneficiary failed");
            let beneficiary_funds: BeneficiaryResponse =
                from_binary(&beneficiary_query_resp).unwrap();
            assert_eq!(
                beneficiary_funds.received_donations[0].amount,
                Uint128::from(45u32)
            );
        }
    }
    #[test]
    fn test_tracking_of_deleted_and_restored_beneficiaries() {
        // Instantiating smart contract
        let mut deps = mock_dependencies_with_balance(&coins(0, "token"));
        let msg = InstantiateMsg {
            only_owner_can_register_beneficiary: false,
        };
        let owner_info = mock_info("owner", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

        // Create one beneficiary
        let beneficiary_info = mock_info("beneficiary1", &coins(1, "token"));

        // Register beneficiary
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            beneficiary_info.clone(),
            ExecuteMsg::RegisterBeneficiary {},
        )
        .expect("register beneficiary failed");

        // Donate 1000 tokens
        let donator1_info = mock_info("donator1", &coins(1000, "token"));
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            donator1_info,
            ExecuteMsg::AddToPot {},
        )
        .expect("failed to donate tokens");

        // Assert that funds have been correctly donated
        let beneficiary_query_resp = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary_info.sender.to_string(),
            },
        )
        .expect("query funds of beneficiary failed");
        let beneficiary_funds: BeneficiaryResponse = from_binary(&beneficiary_query_resp).unwrap();
        let total_funds: Uint128 = beneficiary_funds
            .received_donations
            .iter()
            .map(|funds| funds.amount)
            .sum();
        assert_eq!(total_funds, Uint128::from(1000u32));

        // Remove beneficiary
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            beneficiary_info.clone(),
            ExecuteMsg::RemoveBeneficiary {},
        )
        .expect("removing beneficiary failed");

        // Funds should be in removed beneficiaries
        let removed_beneficiary_query_resp = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetRemovedBeneficiary {
                beneficiary: beneficiary_info.sender.to_string(),
            },
        )
        .expect("query funds of removed beneficiary failed");
        let removed_beneficiary_funds: BeneficiaryResponse =
            from_binary(&removed_beneficiary_query_resp).unwrap();
        let total_funds: Uint128 = removed_beneficiary_funds
            .received_donations
            .iter()
            .map(|funds| funds.amount)
            .sum();
        assert_eq!(total_funds, Uint128::from(1000u32));

        // Register beneficiary again
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            beneficiary_info.clone(),
            ExecuteMsg::RegisterBeneficiary {},
        )
        .expect("register beneficiary failed");

        // Check that funds logs have been restored
        let beneficiary_query_resp = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary_info.sender.to_string(),
            },
        )
        .expect("query funds of beneficiary failed");
        let beneficiary_funds: BeneficiaryResponse = from_binary(&beneficiary_query_resp).unwrap();
        let total_funds: Uint128 = beneficiary_funds
            .received_donations
            .iter()
            .map(|funds| funds.amount)
            .sum();
        assert_eq!(total_funds, Uint128::from(1000u32));

        // Donate 500 tokens
        let donator1_info = mock_info("donator1", &coins(500, "token"));
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            donator1_info,
            ExecuteMsg::AddToPot {},
        )
        .expect("failed to donate tokens");

        // Check that funds have been received properly
        let beneficiary_query_resp = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary_info.sender.to_string(),
            },
        )
        .expect("query funds of beneficiary failed");
        let beneficiary_funds: BeneficiaryResponse = from_binary(&beneficiary_query_resp).unwrap();
        let total_funds: Uint128 = beneficiary_funds
            .received_donations
            .iter()
            .map(|funds| funds.amount)
            .sum();
        assert_eq!(total_funds, Uint128::from(1500u32));

        // Remove beneficiary again (as owner)
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            owner_info.clone(),
            ExecuteMsg::RemoveBeneficiaryAsOwner {
                beneficiary: beneficiary_info.sender.to_string(),
            },
        )
        .expect("removing beneficiary as owner failed");

        // Query one last time from removed beneficiaries
        let removed_beneficiary_query_resp = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetRemovedBeneficiary {
                beneficiary: beneficiary_info.sender.to_string(),
            },
        )
        .expect("query funds of removed beneficiary failed");
        let removed_beneficiary_funds: BeneficiaryResponse =
            from_binary(&removed_beneficiary_query_resp).unwrap();
        let total_funds: Uint128 = removed_beneficiary_funds
            .received_donations
            .iter()
            .map(|funds| funds.amount)
            .sum();
        assert_eq!(total_funds, Uint128::from(1500u32));
    }
    #[test]
    fn test_donations_are_not_received_by_removed_beneficiaries() {
        // Instantiating smart contract
        let mut deps = mock_dependencies_with_balance(&coins(0, "token"));
        let msg = InstantiateMsg {
            only_owner_can_register_beneficiary: false,
        };
        let owner_info = mock_info("owner", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), owner_info.clone(), msg).unwrap();

        // Create two beneficiaries
        let beneficiary1_info = mock_info("beneficiary1", &coins(1, "token"));
        let beneficiary2_info = mock_info("beneficiary2", &coins(1, "token"));

        // Register beneficiaries
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            beneficiary1_info.clone(),
            ExecuteMsg::RegisterBeneficiary {},
        )
        .expect("register beneficiary1 failed");
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            beneficiary2_info.clone(),
            ExecuteMsg::RegisterBeneficiary {},
        )
        .expect("register beneficiary failed");

        // Remove beneficiary1
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            owner_info.clone(),
            ExecuteMsg::RemoveBeneficiaryAsOwner {
                beneficiary: beneficiary1_info.sender.to_string(),
            },
        )
        .expect("removing beneficiary as owner failed");

        // Donate funds
        let donator1_info = mock_info("donator1", &coins(500, "token"));
        let _res = execute(
            deps.as_mut(),
            mock_env(),
            donator1_info,
            ExecuteMsg::AddToPot {},
        )
        .expect("failed to donate tokens");

        // Assert that beneficiary1 received none of the funds
        let _res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary1_info.sender.to_string(),
            },
        )
        .expect_err("should not be able to get beneficiary1 as a regular one");
        let removed_beneficiary1_query_resp = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetRemovedBeneficiary {
                beneficiary: beneficiary1_info.sender.to_string(),
            },
        )
        .expect("query funds of removed beneficiary failed");
        let removed_beneficiary1_funds: BeneficiaryResponse =
            from_binary(&removed_beneficiary1_query_resp).unwrap();
        let total_funds: Uint128 = removed_beneficiary1_funds
            .received_donations
            .iter()
            .map(|funds| funds.amount)
            .sum();
        assert_eq!(total_funds, Uint128::from(0u32));

        // Check that beneficiary2 received all funds
        let beneficiary2_query_resp = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBeneficiary {
                beneficiary: beneficiary2_info.sender.to_string(),
            },
        )
        .expect("query funds of removed beneficiary failed");
        let beneficiary2_funds: BeneficiaryResponse =
            from_binary(&beneficiary2_query_resp).unwrap();
        let total_funds: Uint128 = beneficiary2_funds
            .received_donations
            .iter()
            .map(|funds| funds.amount)
            .sum();
        assert_eq!(total_funds, Uint128::from(500u32));
    }
}
