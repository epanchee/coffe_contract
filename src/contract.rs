#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20Coin;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{ADMIN, BALANCES, BEVERAGES};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:my-first-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    create_accounts(&mut deps, &msg.initial_balances)?;
    BALANCES.save(deps.storage, &_env.contract.address, &Uint128::zero())?;
    ADMIN.save(deps.storage, &msg.admin)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

pub fn create_accounts(deps: &mut DepsMut, accounts: &[Cw20Coin]) -> StdResult<()> {
    for row in accounts {
        let address = deps.api.addr_validate(&row.address)?;
        BALANCES.save(deps.storage, &address, &row.amount)?;
    }
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateBeverage { bev_type, price } => {
            update_beverage(deps, info, &bev_type, price)
        }
        ExecuteMsg::RefillBeverage { bev_type, amount } => {
            refill_beverage(deps, info, &bev_type, amount)
        }
        ExecuteMsg::Purchase { bev_type } => purchase(deps, _env, info, &bev_type),
        ExecuteMsg::WithdrawIncome {} => withdraw_income(deps, _env, info),
    }
}

fn update_beverage(
    deps: DepsMut,
    info: MessageInfo,
    bev_type: &str,
    price: Uint128,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender.ne(&admin) {
        return Err(ContractError::Unauthorized {});
    }

    BEVERAGES.update(
        deps.storage,
        bev_type,
        |stat_opt| -> Result<_, ContractError> {
            let mut stat = stat_opt.unwrap_or_default();
            stat.price = price;
            Ok(stat)
        },
    )?;

    Ok(Response::new()
        .add_attribute("beverage_type", bev_type)
        .add_attribute("price", price))
}

fn refill_beverage(
    deps: DepsMut,
    info: MessageInfo,
    bev_type: &str,
    amount: u8,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender.ne(&admin) {
        return Err(ContractError::Unauthorized {});
    }

    BEVERAGES.may_load(deps.storage, bev_type)?;

    BEVERAGES.update(
        deps.storage,
        bev_type,
        |stat_opt| -> Result<_, ContractError> { stat_opt.unwrap().refill(amount) },
    )?;

    Ok(Response::new()
        .add_attribute("action", "refill")
        .add_attribute("beverage_type", bev_type)
        .add_attribute("amount", amount.to_string()))
}

fn purchase(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    bev_type: &str,
) -> Result<Response, ContractError> {
    let price = BEVERAGES.load(deps.storage, bev_type)?.price;

    BEVERAGES.may_load(deps.storage, bev_type)?;

    BALANCES.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> Result<_, ContractError> {
            balance
                .unwrap()
                .checked_sub(price)
                .or(Err(ContractError::NotEnoughCoins {}))
        },
    )?;

    BALANCES.update(
        deps.storage,
        &_env.contract.address,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap() + price) },
    )?;

    BEVERAGES.update(deps.storage, bev_type, |stat_opt| stat_opt.unwrap().sell())?;

    Ok(Response::default())
}

fn withdraw_income(deps: DepsMut, _env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender.ne(&admin) {
        return Err(ContractError::Unauthorized {});
    }

    let cur_contract_balance = BALANCES.load(deps.storage, &_env.contract.address)?;

    BALANCES.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default() + cur_contract_balance)
        },
    )?;

    BALANCES.update(
        deps.storage,
        &_env.contract.address,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default() - cur_contract_balance)
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "winthdraw_income")
        .add_attribute("amount", cur_contract_balance)
        .add_attribute("recipient", admin))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => {
            let address = deps.api.addr_validate(&address)?;
            let balance = BALANCES
                .may_load(deps.storage, &address)?
                .unwrap_or_default();
            Ok(to_binary(&cw20::BalanceResponse { balance })?)
        }
        QueryMsg::BeverageStat { bev_type } => {
            let bev_stat = BEVERAGES.load(deps.storage, &bev_type)?;
            Ok(to_binary(&bev_stat)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::state::BeverageStat;

    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{from_binary, Addr, StdError};

    fn do_intantiate(deps: DepsMut, info: MessageInfo) -> Response {
        let initial_balances = vec![Cw20Coin {
            address: "addr0".to_string(),
            amount: Uint128::from(10_u16),
        }];

        let msg = InstantiateMsg {
            admin: Addr::unchecked("admin".to_string()),
            initial_balances,
        };

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps, mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        res
    }

    fn query_balance(deps: DepsMut, addr: &str) -> cw20::BalanceResponse {
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Balance {
                address: addr.to_string(),
            },
        )
        .unwrap();
        from_binary(&res).unwrap()
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("admin", &[]);

        do_intantiate(deps.as_mut(), info);

        let value: cw20::BalanceResponse = query_balance(deps.as_mut(), "addr0");
        assert_eq!(Uint128::from(10_u32), value.balance);
    }

    #[test]
    fn test_update_beverage() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("admin", &[]);

        do_intantiate(deps.as_mut(), info);

        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("admin", &[]),
            ExecuteMsg::UpdateBeverage {
                bev_type: "americano".to_string(),
                price: Uint128::from(2_u16),
            },
        )
        .unwrap();

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::BeverageStat {
                bev_type: "americano".to_string(),
            },
        )
        .unwrap();

        let value: BeverageStat = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(2_u16), value.price)
    }

    #[test]
    fn test_refill_beverage() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("admin", &[]);

        do_intantiate(deps.as_mut(), info);

        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("admin", &[]),
            ExecuteMsg::UpdateBeverage {
                bev_type: "americano".to_string(),
                price: Uint128::from(2_u16),
            },
        )
        .unwrap();

        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("admin", &[]),
            ExecuteMsg::RefillBeverage {
                bev_type: "americano".to_string(),
                amount: 20,
            },
        )
        .unwrap();

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::BeverageStat {
                bev_type: "americano".to_string(),
            },
        )
        .unwrap();

        let value: BeverageStat = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(2_u16), value.price);
        assert_eq!(20, value.amount);

        // trying to exceed 50 items
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("admin", &[]),
            ExecuteMsg::RefillBeverage {
                bev_type: "americano".to_string(),
                amount: 31,
            },
        )
        .unwrap_err();

        assert!(matches!(res, ContractError::BeverageNumberExceed {}))
    }

    #[test]
    fn test_purchase() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("admin", &[]);
        let env = mock_env();

        do_intantiate(deps.as_mut(), info);

        execute(
            deps.as_mut(),
            env.clone(),
            mock_info("admin", &[]),
            ExecuteMsg::UpdateBeverage {
                bev_type: "americano".to_string(),
                price: Uint128::from(2_u16),
            },
        )
        .unwrap();

        execute(
            deps.as_mut(),
            env.clone(),
            mock_info("admin", &[]),
            ExecuteMsg::RefillBeverage {
                bev_type: "americano".to_string(),
                amount: 1,
            },
        )
        .unwrap();

        execute(
            deps.as_mut(),
            env.clone(),
            mock_info("addr0", &[]),
            ExecuteMsg::Purchase {
                bev_type: "americano".to_string(),
            },
        )
        .unwrap();

        let value: cw20::BalanceResponse = query_balance(deps.as_mut(), "addr0");
        assert_eq!(Uint128::from(8_u32), value.balance);
        let value: cw20::BalanceResponse =
            query_balance(deps.as_mut(), &env.contract.address.to_string());
        assert_eq!(Uint128::from(2_u32), value.balance);

        // trying to purchase one more americano but it should be over
        let res = execute(
            deps.as_mut(),
            env.clone(),
            mock_info("addr0", &[]),
            ExecuteMsg::Purchase {
                bev_type: "americano".to_string(),
            },
        ).unwrap_err();

        assert!(matches!(res, ContractError::BeverageIsOver {}));

        // increase the price for americano to 9 coins
        execute(
            deps.as_mut(),
            env.clone(),
            mock_info("admin", &[]),
            ExecuteMsg::UpdateBeverage {
                bev_type: "americano".to_string(),
                price: Uint128::from(9_u16),
            },
        )
        .unwrap();

        // trying to purchase one more americano with price 9 coins
        let res = execute(
            deps.as_mut(),
            env,
            mock_info("addr0", &[]),
            ExecuteMsg::Purchase {
                bev_type: "americano".to_string(),
            },
        )
        .unwrap_err();

        assert!(matches!(res, ContractError::NotEnoughCoins {}))
    }

    #[test]
    fn test_withraw() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("admin", &[]);
        let env = mock_env();

        do_intantiate(deps.as_mut(), info);

        execute(
            deps.as_mut(),
            env.clone(),
            mock_info("admin", &[]),
            ExecuteMsg::UpdateBeverage {
                bev_type: "americano".to_string(),
                price: Uint128::from(2_u16),
            },
        )
        .unwrap();

        execute(
            deps.as_mut(),
            env.clone(),
            mock_info("admin", &[]),
            ExecuteMsg::RefillBeverage {
                bev_type: "americano".to_string(),
                amount: 1,
            },
        )
        .unwrap();

        execute(
            deps.as_mut(),
            env.clone(),
            mock_info("addr0", &[]),
            ExecuteMsg::Purchase {
                bev_type: "americano".to_string(),
            },
        )
        .unwrap();

        execute(
            deps.as_mut(),
            env.clone(),
            mock_info("admin", &[]),
            ExecuteMsg::WithdrawIncome {},
        )
        .unwrap();

        let value: cw20::BalanceResponse = query_balance(deps.as_mut(), "admin");
        assert_eq!(Uint128::from(2_u32), value.balance);
        let value: cw20::BalanceResponse =
            query_balance(deps.as_mut(), &env.contract.address.to_string());
        assert_eq!(Uint128::from(0_u32), value.balance);

        let res = execute(
            deps.as_mut(),
            env,
            mock_info("random_user", &[]),
            ExecuteMsg::WithdrawIncome {},
        )
        .unwrap_err();

        assert!(matches!(res, ContractError::Unauthorized {}))
    }

    #[test]
    fn test_not_found() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("admin", &[]);

        do_intantiate(deps.as_mut(), info);

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("addr0", &[]),
            ExecuteMsg::Purchase {
                bev_type: "americano".to_string(),
            },
        )
        .unwrap_err();

        assert!(matches!(
            res,
            ContractError::Std(StdError::NotFound { kind: _ })
        ))
    }
}
