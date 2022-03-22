#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_binary, BankMsg, Binary, Coin, Deps, DepsMut, Empty, Env, MessageInfo, Response,
    StdResult, Uint128,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{State, BENEFICIARIES, STATE};

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

pub fn add_to_pot(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut amount_of_beneficiaries = 0;
    for _beneficiary in BENEFICIARIES.keys(deps.storage, None, None, cosmwasm_std::Order::Ascending)
    {
        amount_of_beneficiaries += 1;
    }
    let funds_for_each = split_coins_into_parts(&info.funds, amount_of_beneficiaries);
    // TODO: Add DONORS informations
    /*DONORS.update(
        deps.storage,
        info.sender,
        |donor| -> Result<_, ContractError> {
            // TODO: Extend vector with funds
            Ok(info.funds)
        },
    );*/

    let mut response: Response<Empty> = Response::new();

    let mut beneficiaries_out: Vec<String> = Vec::new();
    // TODO: Add beneficiary stuff
    for (beneficiary, coin_part) in BENEFICIARIES
        .keys(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .zip(funds_for_each)
    {
        let beneficiary_addr = beneficiary.expect("invalid beneficiary address");
        beneficiaries_out.push(beneficiary_addr.clone().into_string());
        response = response.add_message(BankMsg::Send {
            amount: coins(coin_part[0].amount.u128(), coin_part[0].denom.clone()),
            to_address: beneficiary_addr.clone().into_string(),
        });
    }
    let beneficiaries_as_str = format!("{:?}", beneficiaries_out);
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
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    /*match msg {
        QueryMsg::GetTotal {} => to_binary(&query_total(deps)?),
        QueryMsg::GetDonor { donor } => to_binary(&query_donor(deps, donor)?),
    }*/
    to_binary(&0)
}

/*fn query_donor(deps: Deps, donor: Addr) -> StdResult<PotDonorResponse> {
    let state = STATE.load(deps.storage)?;
    let total = Uint128::new(0);
    if let Some(donor_coins) = state.donors.get(&donor) {
        for donor_coin in donor_coins {
            total.saturating_add(donor_coin.amount);
        }
    }

    Ok(PotDonorResponse { total: total })
}

fn query_total(deps: Deps) -> StdResult<PotTotalResponse> {
    let state = STATE.load(deps.storage)?;
    let total = Uint128::new(0);
    for donor_coins in state.donors.values() {
        for donor_coin in donor_coins {
            total.saturating_add(donor_coin.amount);
        }
    }
    Ok(PotTotalResponse { total: total })
}*/

/*#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: PotTotalResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: PotTotalResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: PotTotalResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }
}
*/
