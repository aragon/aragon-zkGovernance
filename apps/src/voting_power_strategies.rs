use crate::{Asset, EthHostEvmEnv};
use alloy::{network::Network, providers::Provider, transports::Transport};
use alloy_primitives::{Address, U256};
use async_trait::async_trait;

#[async_trait]
pub trait VotingPowerStrategy<T, N, P, H>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + 'static,
    H: Send + 'static,
{
    async fn process(
        &self,
        env: &mut EthHostEvmEnv<T, N, P, H>,
        account: Address,
        asset: &Asset,
    ) -> U256;
}

mod balance_of;
mod get_past_votes;
pub use balance_of::BalanceOf;
pub use get_past_votes::GetPastVotes;
