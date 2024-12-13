use crate::EthHostEvmEnv;
use alloy::{network::Network, providers::Provider, transports::Transport};
use alloy_primitives::U256;
use async_trait::async_trait;

#[async_trait]
pub trait ExecutionStrategy<T, N, P, H>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + 'static,
    H: Send + 'static,
{
    async fn proof_execution(
        &self,
        env: &mut EthHostEvmEnv<T, N, P, H>,
        total_supply: U256,
        tally: [U256; 3],
    ) -> bool;
}

mod majority_voting;

pub use majority_voting::MajorityVoting;
