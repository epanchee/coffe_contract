use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Beverage max number exceed")]
    BeverageNumberExceed {},

    #[error("The beverage is over")]
    BeverageIsOver {},

    #[error("Not enough coins")]
    NotEnoughCoins {},
}
