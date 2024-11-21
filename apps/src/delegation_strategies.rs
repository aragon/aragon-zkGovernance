use crate::{Asset, EthHostEvmEnv};
use alloy::{network::Network, providers::Provider, transports::Transport};
use alloy_primitives::{Address, Bytes, U256};
use anyhow::Result;
use async_trait::async_trait;
use std::iter::FromIterator;

pub struct Delegation {
    pub delegate: Address,
    pub ratio: U256,
}

impl FromIterator<(Address, U256)> for Delegation {
    fn from_iter<I: IntoIterator<Item = (Address, U256)>>(iter: I) -> Self {
        let mut iter = iter.into_iter();

        // Take the first tuple from the iterator to create the struct
        if let Some((delegate, ratio)) = iter.next() {
            println!("delegate: {}, ratio: {}", delegate, ratio);
            Delegation { delegate, ratio }
        } else {
            panic!("Iterator is empty, cannot create Delegation");
        }
    }
}

#[async_trait]
pub trait DelegationStrategy<T, N, P, H>
where
    T: Transport + Clone + Send + Sync,
    N: Network + Send + Sync,
    P: Provider<T, N> + Send + Sync + 'static,
    H: Clone + Send + Sync + 'static,
{
    async fn process(
        &self,
        env: &mut EthHostEvmEnv<T, N, P, H>,
        account: Address,
        asset: &Asset,
        additional_data: Bytes,
    ) -> Result<Vec<Delegation>>;
}

mod split_delegation;

pub use split_delegation::SplitDelegation;
