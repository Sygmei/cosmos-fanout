#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, StdResult, Storage, Uint128,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, PotDonatorResponse, QueryMsg};
use crate::state::{State, BENEFICIARIES, DONATORS, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cosmos-fanout";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let state = State {
        owner: info.sender.clone(),
    };
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
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
        ExecuteMsg::RegisterBeneficiary {} => register_beneficiary(deps, info),
        ExecuteMsg::AddToPot {} => add_to_pot(deps, info),
    }
}

pub fn register_beneficiary(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    if BENEFICIARIES.has(deps.storage, info.sender.clone()) {
        return Err(ContractError::AlreadyABeneficiary {});
    }
    let result = BENEFICIARIES.save(deps.storage, info.sender, &mut Vec::new());
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
        QueryMsg::GetDonator { donator } => to_binary(&query_donator(deps, donator)?),
    }
}

fn query_donator(deps: Deps, donator: Addr) -> StdResult<PotDonatorResponse> {
    if let Ok(donator_infos) = DONATORS.load(deps.storage, donator.clone()) {
        return Ok(PotDonatorResponse {
            donator: donator,
            donations: donator_infos,
        });
    }

    return Ok(PotDonatorResponse {
        donator: donator,
        donations: [].to_vec(),
    });
}
