#![cfg(test)]

use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{coins, Addr, Empty, StdError, Uint128};
use cw20::{Cw20Coin, Cw20Contract};
use cw_multi_test::{App, BankKeeper, Contract, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, query};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::BeverageStat;
use crate::ContractError;

fn mock_app() -> App {
    let env = mock_env();
    let api = MockApi::default();
    let bank = BankKeeper::new();

    App::new(api, env.block, bank, MockStorage::new())
}

pub fn contract_coffee() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(execute, instantiate, query);
    Box::new(contract)
}

pub fn new_item_msg(name: &str, price: u16) -> ExecuteMsg {
    ExecuteMsg::UpdateBeverage {
        bev_type: String::from(name),
        price: Uint128::from(price),
    }
}

pub fn refill_beverage_msg(name: &str, amount: u8) -> ExecuteMsg {
    ExecuteMsg::RefillBeverage {
        bev_type: String::from(name),
        amount,
    }
}

pub fn purchase_msg(name: &str) -> ExecuteMsg {
    ExecuteMsg::Purchase {
        bev_type: String::from(name),
    }
}

pub fn query_bev_stat(router: &mut App, sender: Addr, name: &str) -> BeverageStat {
    router
        .wrap()
        .query_wasm_smart(
            sender,
            &QueryMsg::BeverageStat {
                bev_type: String::from(name),
            },
        )
        .unwrap()
}

pub fn create_and_refill(
    router: &mut App,
    sender: Addr,
    contract_addr: Addr,
    name: &str,
    price: u16,
    amount: u8,
) {
    let new_msg = new_item_msg(name, price);
    router
        .execute_contract(sender.clone(), contract_addr.clone(), &new_msg, &[])
        .unwrap();
    let refill_msg = refill_beverage_msg(name, amount);
    router
        .execute_contract(sender, contract_addr, &refill_msg, &[])
        .unwrap();
}

#[test]
fn test_vending_machine() {
    let mut router = mock_app();

    // set personal balance
    let admin = Addr::unchecked("admin");
    let init_funds = coins(100, "COFFEETOKEN");
    router.init_bank_balance(&admin, init_funds).unwrap();

    // set up contract
    let contract_id = router.store_code(contract_coffee());
    let msg = InstantiateMsg {
        initial_balances: vec![Cw20Coin {
            address: "addr0".to_string(),
            amount: Uint128::from(10_u16),
        }],
    };
    let cash_addr = router
        .instantiate_contract(
            contract_id,
            admin.clone(),
            &msg,
            &[],
            "Vending-machine",
            None,
        )
        .unwrap();

    // set up cw20 helpers
    let cash = Cw20Contract(cash_addr.clone());

    // ensure our balance
    let owner_balance = cash.balance(&router, admin.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::zero());

    create_and_refill(
        &mut router,
        admin.clone(),
        cash_addr.clone(),
        "americano",
        2,
        1,
    );
    create_and_refill(
        &mut router,
        admin.clone(),
        cash_addr.clone(),
        "cappuccino",
        4,
        49,
    );
    create_and_refill(&mut router, admin.clone(), cash_addr.clone(), "latte", 5, 2);

    let customer = Addr::unchecked("addr0");

    router
        .execute_contract(
            customer.clone(),
            cash_addr.clone(),
            &purchase_msg("americano"),
            &[],
        )
        .unwrap();
    router
        .execute_contract(
            customer.clone(),
            cash_addr.clone(),
            &purchase_msg("latte"),
            &[],
        )
        .unwrap();

    // check errors
    let err = router
        .execute_contract(
            customer.clone(),
            cash_addr.clone(),
            &purchase_msg("americano"),
            &[],
        )
        .unwrap_err();

    assert!(matches!(
        err.downcast_ref().unwrap(),
        ContractError::BeverageIsOver {}
    ));

    let err = router
        .execute_contract(
            customer.clone(),
            cash_addr.clone(),
            &purchase_msg("cappuccino"),
            &[],
        )
        .unwrap_err();

    assert!(matches!(
        err.downcast_ref().unwrap(),
        ContractError::NotEnoughCoins {}
    ));

    // triggering overflow (above 50) since 49 items are there
    let refill_msg = refill_beverage_msg("cappuccino", 2);
    let err = router
        .execute_contract(admin.clone(), cash_addr.clone(), &refill_msg, &[])
        .unwrap_err();
    assert!(matches!(
        err.downcast_ref().unwrap(),
        ContractError::BeverageNumberExceed {}
    ));

    // trying to refill item which does not exist
    let err = router
        .execute_contract(
            customer.clone(),
            cash_addr.clone(),
            &purchase_msg("water"),
            &[],
        )
        .unwrap_err();

    assert!(matches!(
        err.downcast_ref().unwrap(),
        ContractError::Std(StdError::NotFound { kind: _ })
    ));

    // check protected endpoints
    let err = router
        .execute_contract(customer.clone(), cash_addr.clone(), &refill_msg, &[])
        .unwrap_err();
    assert!(matches!(
        err.downcast_ref().unwrap(),
        ContractError::Unauthorized {}
    ));

    let beer_msg = new_item_msg("beer", 100);
    let err = router
        .execute_contract(customer.clone(), cash_addr.clone(), &beer_msg, &[])
        .unwrap_err();
    assert!(matches!(
        err.downcast_ref().unwrap(),
        ContractError::Unauthorized {}
    ));

    let err = router
        .execute_contract(
            customer.clone(),
            cash_addr.clone(),
            &ExecuteMsg::WithdrawIncome {},
            &[],
        )
        .unwrap_err();
    assert!(matches!(
        err.downcast_ref().unwrap(),
        ContractError::Unauthorized {}
    ));

    // ensure balances
    let admin_balance = cash.balance(&router, admin.clone()).unwrap();
    assert_eq!(admin_balance, Uint128::zero());
    let customer_balance = cash.balance(&router, customer.clone()).unwrap();
    assert_eq!(customer_balance, Uint128::from(3_u16));
    let contract_balance = cash.balance(&router, cash_addr.clone()).unwrap();
    assert_eq!(contract_balance, Uint128::from(7_u16));

    // ensure beverages stats
    let americano = query_bev_stat(&mut router, cash_addr.clone(), "americano");
    assert_eq!(Uint128::from(2_u16), americano.price);
    assert_eq!(0, americano.amount);

    let latte = query_bev_stat(&mut router, cash_addr.clone(), "latte");
    assert_eq!(Uint128::from(5_u16), latte.price);
    assert_eq!(1, latte.amount);

    let latte = query_bev_stat(&mut router, cash_addr.clone(), "cappuccino");
    assert_eq!(Uint128::from(4_u16), latte.price);
    assert_eq!(49, latte.amount);

    // withdraw income and ensure balances again
    router
        .execute_contract(
            admin.clone(),
            cash_addr.clone(),
            &ExecuteMsg::WithdrawIncome {},
            &[],
        )
        .unwrap();

    let admin_balance = cash.balance(&router, admin.clone()).unwrap();
    assert_eq!(admin_balance, Uint128::from(7_u16));
    let customer_balance = cash.balance(&router, customer).unwrap();
    assert_eq!(customer_balance, Uint128::from(3_u16));
    let contract_balance = cash.balance(&router, cash_addr).unwrap();
    assert_eq!(contract_balance, Uint128::from(0_u16));
}
