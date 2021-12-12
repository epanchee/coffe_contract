#![cfg(test)]

use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{coins, to_binary, Addr, Empty, StdResult, Uint128};
use cw20::{Cw20Coin, Cw20Contract, Cw20ExecuteMsg};
use cw_multi_test::{App, AppResponse, BankKeeper, Contract, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, query};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
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
    let owner = Addr::unchecked("admin");
    let init_funds = coins(100, "COFFEETOKEN");
    router.init_bank_balance(&owner, init_funds).unwrap();

    // set up contract
    let contract_id = router.store_code(contract_coffee());
    let msg = InstantiateMsg {
        admin: owner.clone(),
        initial_balances: vec![Cw20Coin {
            address: "addr0".to_string(),
            amount: Uint128::from(10_u16),
        }],
    };
    let cash_addr = router
        .instantiate_contract(contract_id, owner.clone(), &msg, &[], "CASH", None)
        .unwrap();

    // set up cw20 helpers
    let cash = Cw20Contract(cash_addr.clone());

    // ensure our balance
    let owner_balance = cash.balance(&router, owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::zero());

    create_and_refill(
        &mut router,
        owner.clone(),
        cash_addr.clone(),
        "americano",
        2,
        2,
    );
    create_and_refill(
        &mut router,
        owner.clone(),
        cash_addr.clone(),
        "cappuccino",
        4,
        1,
    );
    create_and_refill(&mut router, owner.clone(), cash_addr.clone(), "latte", 5, 2);

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
    let err = router.execute_contract(
        customer,
        cash_addr.clone(),
        &purchase_msg("cappuccino"),
        &[],
    ).unwrap_err();

    assert!(matches!(err, ContractError::NotEnoughCoins {}))

    // // send some tokens to create an escrow
    // let arb = Addr::unchecked("arbiter");
    // let ben = String::from("beneficiary");
    // let id = "demo".to_string();
    // let create_msg = ReceiveMsg::Create(CreateMsg {
    //     id: id.clone(),
    //     arbiter: arb.to_string(),
    //     recipient: ben.clone(),
    //     end_height: None,
    //     end_time: None,
    //     cw20_whitelist: None,
    // });
    // let send_msg = Cw20ExecuteMsg::Send {
    //     contract: escrow_addr.to_string(),
    //     amount: Uint128::new(1200),
    //     msg: to_binary(&create_msg).unwrap(),
    // };
    // let res = router
    //     .execute_contract(owner.clone(), cash_addr.clone(), &send_msg, &[])
    //     .unwrap();
    // assert_eq!(4, res.events.len());
    // println!("{:?}", res.events);

    // assert_eq!(res.events[0].ty.as_str(), "execute");
    // let cw20_attr = res.custom_attrs(1);
    // println!("{:?}", cw20_attr);
    // assert_eq!(4, cw20_attr.len());

    // assert_eq!(res.events[2].ty.as_str(), "execute");
    // let escrow_attr = res.custom_attrs(3);
    // println!("{:?}", escrow_attr);
    // assert_eq!(2, escrow_attr.len());

    // // ensure balances updated
    // let owner_balance = cash.balance(&router, owner.clone()).unwrap();
    // assert_eq!(owner_balance, Uint128::new(3800));
    // let escrow_balance = cash.balance(&router, escrow_addr.clone()).unwrap();
    // assert_eq!(escrow_balance, Uint128::new(1200));

    // // ensure escrow properly created
    // let details: DetailsResponse = router
    //     .wrap()
    //     .query_wasm_smart(&escrow_addr, &QueryMsg::Details { id: id.clone() })
    //     .unwrap();
    // assert_eq!(id, details.id);
    // assert_eq!(arb, details.arbiter);
    // assert_eq!(ben, details.recipient);
    // assert_eq!(
    //     vec![Cw20Coin {
    //         address: cash_addr.to_string(),
    //         amount: Uint128::new(1200)
    //     }],
    //     details.cw20_balance
    // );

    // // release escrow
    // let approve_msg = ExecuteMsg::Approve { id };
    // let _ = router
    //     .execute_contract(arb, escrow_addr.clone(), &approve_msg, &[])
    //     .unwrap();

    // // ensure balances updated - release to ben
    // let owner_balance = cash.balance(&router, owner).unwrap();
    // assert_eq!(owner_balance, Uint128::new(3800));
    // let escrow_balance = cash.balance(&router, escrow_addr).unwrap();
    // assert_eq!(escrow_balance, Uint128::zero());
    // let ben_balance = cash.balance(&router, ben).unwrap();
    // assert_eq!(ben_balance, Uint128::new(1200));
}
