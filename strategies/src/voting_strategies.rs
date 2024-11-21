use crate::{Asset, GuestEvmEnv};
use alloy_primitives::{Address, U256};

pub trait VotingPowerStrategy {
    fn process(&self, env: &GuestEvmEnv, account: Address, asset: &Asset) -> U256;

    fn get_supply(&self, env: &GuestEvmEnv, asset: &Asset) -> U256;
}

mod balance_of;
mod get_past_votes;

pub use balance_of::BalanceOf;
pub use get_past_votes::GetPastVotes;
