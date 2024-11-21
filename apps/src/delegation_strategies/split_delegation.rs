use super::DelegationStrategy;
use crate::Delegation;
use crate::{Asset, EthHostEvmEnv};
use alloy::{network::Network, providers::Provider, transports::Transport};
use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::sol;
use anyhow::{bail, Result};
use async_trait::async_trait;
use risc0_steel::Contract;

sol! {
    /// ERC-20 balance function signature.
    interface DelegateRegistry {
        struct Delegation {
            bytes32 delegate;
            uint256 ratio;
        }
        function getDelegation(string memory context, address account) public view returns (Delegation[] memory delegations, uint256 expirationTimestamp);
    }
}

pub struct SplitDelegation;

#[async_trait]
impl<T, N, P, H> DelegationStrategy<T, N, P, H> for SplitDelegation
where
    T: Transport + Clone + Send + Sync,
    N: Network + Send + Sync,
    P: Provider<T, N> + Send + Sync + 'static,
    H: Clone + Send + Sync + 'static,
{
    async fn process(
        &self,
        env: &mut EthHostEvmEnv<T, N, P, H>, // Mutable reference passed
        account: Address,
        asset: &Asset,
        additional_data: Bytes,
    ) -> Result<Vec<Delegation>> {
        // Ensure the length of the input bytes is a multiple of 20
        if additional_data.len() % 20 != 0 {
            bail!("Input byte vector is not a valid length for Address conversion");
        }

        // Collect chunks of 20 bytes and convert them into `Address`
        let delegations: Vec<Address> = additional_data
            .chunks_exact(20) // Split the input bytes into chunks of 20
            .map(|chunk| Address::from_slice(chunk)) // Convert each chunk into an `Address`
            .collect();
        println!("Input Delegations: {:?}", delegations);

        let context = asset.contract.clone();
        let mut account_delegates = Vec::new();

        for potential_delegate in delegations {
            let context_clone = context.to_string();
            let asset_contract_clone = asset.delegation.contract.clone();
            let result = process_delegate(
                env,
                asset_contract_clone,
                context_clone,
                potential_delegate,
                account,
            )
            .await;

            account_delegates.push(result);
        }

        // Check if any results are `None`
        if account_delegates.iter().any(|d| d.is_none()) {
            bail!("One or more delegations are invalid");
        } else {
            Ok(account_delegates.into_iter().map(|d| d.unwrap()).collect())
        }
    }
}

// Extract the async logic into a separate function
async fn process_delegate<T, N, P, H>(
    env: &mut EthHostEvmEnv<T, N, P, H>,
    asset_contract: Address,
    context: String,
    potential_delegate: Address,
    account: Address,
) -> Option<Delegation>
where
    T: Transport + Clone + Send + Sync,
    N: Network + Send + Sync,
    P: Provider<T, N> + Send + Sync + 'static,
    H: Clone + Send + Sync + 'static,
{
    // Create the delegations contract
    let mut delegations_contract = Contract::preflight(asset_contract, env);

    // Build the call
    let potential_delegate_delegations_call = DelegateRegistry::getDelegationCall {
        context,
        account: potential_delegate,
    };

    // Await the call result
    let potential_delegate_delegations = delegations_contract
        .call_builder(&potential_delegate_delegations_call)
        .call()
        .await
        .unwrap();

    println!(
        "Potential Delegate Delegations: {:?}, {:?}",
        potential_delegate_delegations.delegations[0].delegate,
        potential_delegate_delegations.delegations[0].ratio
    );

    if potential_delegate_delegations.delegations.is_empty() {
        return Some(Delegation {
            delegate: potential_delegate,
            ratio: U256::from(1),
        });
    }

    // Find the matching delegation for the account and return a Some(Delegation) if valid
    let total_ratios = potential_delegate_delegations
        .delegations
        .iter()
        .fold(U256::from(0), |acc, d| acc + d.ratio);

    potential_delegate_delegations
        .delegations
        .iter()
        .find(|d| compare_bytes32_to_address(d.delegate, account))
        .map(|d| Delegation {
            delegate: potential_delegate,
            ratio: total_ratios / d.ratio,
        })
}

fn compare_bytes32_to_address(bytes32: alloy_primitives::FixedBytes<32>, address: Address) -> bool {
    // Extract the last 20 bytes of the bytes32 (rightmost part of the bytes32)
    let bytes = bytes32.as_slice();
    let last_20_bytes = &bytes[12..]; // From index 12 to the end (20 bytes)

    // Compare the last 20 bytes to the address bytes
    last_20_bytes == address
}
