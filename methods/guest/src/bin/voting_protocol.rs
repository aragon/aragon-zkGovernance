#![allow(unused_doc_comments)]
#![no_main]

use std::str::FromStr;

use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::{sol, SolValue};
use hex::FromHex;
use risc0_steel::{
    ethereum::{EthEvmInput, ETH_SEPOLIA_CHAIN_SPEC},
    Commitment, Contract,
};
use risc0_zkvm::guest::env;

use k256::{
    ecdsa::{RecoveryId, Signature, VerifyingKey},
    elliptic_curve::sec1::ToEncodedPoint,
    PublicKey,
};
use tiny_keccak::{Hasher, Keccak};

risc0_zkvm::guest::entry!(main);

/// Specify the function to call using the [`sol!`] macro.
/// This parses the Solidity syntax to generate a struct that implements the `SolCall` trait.
sol! {
    interface ConfigContract {
        function votingProtocolConfig(uint256 proposal_id) external view returns (string memory);
    }
}

/// ABI encodable journal data.
sol! {
    struct Journal {
        Commitment commitment;
        address config_contract;
        uint256 proposal_id;
        address voter;
        uint256 balance;
        uint8 direction;
    }
}

fn to_hex_string(bytes: &[u8]) -> String {
    // Convert each byte to its hexadecimal representation and collect into a single String
    bytes
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<_>()
}

const PREFIX: &str = "\x19Ethereum Signed Message:\n32";

fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut digest = [0u8; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    hasher.finalize(&mut digest);
    digest
}

/// Converts an Ethereum-convention recovery ID to the k256 RecoveryId type.
fn into_recovery_id(v: u8) -> Option<RecoveryId> {
    match v {
        0 => Some(0),
        1 => Some(1),
        27 => Some(0),
        28 => Some(1),
        v if v >= 35 => Some((v - 1) % 2),
        _ => None,
    }
    .and_then(RecoveryId::from_byte)
}

/// Signer address recovery from the (v, r, s) signature components.
///
/// This methods exists to replicate the behavior of `ecrecover` within the EVM.
/// It can only be considered a signature validation is digest is verified to be
/// the hash of a known message.
fn ecrecover(v: u8, rs: [u8; 64], digest: [u8; 32]) -> [u8; 20] {
    let recovery_id = into_recovery_id(v).expect("value for v is invalid");
    let signature = Signature::from_slice(&rs[..]).expect("signature encoding is invalid");
    let recovered_pk: PublicKey =
        VerifyingKey::recover_from_prehash(&digest[..], &signature, recovery_id)
            .expect("signature is invalid")
            .into();

    // Calculate the Ethereum address from the k256 public key.
    let encoded_pk = recovered_pk.to_encoded_point(/* compress = */ false);
    keccak256(&encoded_pk.as_bytes()[1..])[12..]
        .try_into()
        .unwrap()
}

fn hash_vote(
    chain_id: u64,
    dao: Address,
    proposal_id: U256,
    direction: u8,
    balance: U256,
) -> [u8; 32] {
    let message_hash = keccak256(
        &[
            chain_id.to_be_bytes().to_vec(),
            dao.to_vec(),
            proposal_id.to_be_bytes_vec(),
            [direction].to_vec(),
            balance.to_be_bytes_vec(),
        ]
        .concat(),
    );
    let prefixed_message = [PREFIX.as_bytes(), &message_hash].concat();
    keccak256(&prefixed_message)
}

fn main() {
    // Read the input from the guest environment.
    println!("Reading input from the guest environment");
    let input: EthEvmInput = env::read();
    let signature: String = env::read();
    let voter: Address = env::read();
    let dao: Address = env::read();
    let proposal_id: U256 = env::read();
    let direction: u8 = env::read();
    let balance: U256 = env::read();
    let config_contract: Address = env::read();
    let additional_delegation_data: String = env::read();

    let digest = hash_vote(
        ETH_SEPOLIA_CHAIN_SPEC.chain_id(),
        dao,
        proposal_id,
        direction,
        balance,
    );
    let byte_signature = Vec::from_hex(signature).expect("Invalid hex string");

    let v = byte_signature[64];
    let rs = byte_signature[0..64].try_into().unwrap();
    let signature_address = ecrecover(v, rs, digest);

    // Converts the input into a `EvmEnv` for execution. The `with_chain_spec` method is used
    // to specify the chain configuration. It checks that the state matches the state root in the
    // header provided in the input.
    let destination_chain_id = &ETH_SEPOLIA_CHAIN_SPEC;
    let env = input.into_env().with_chain_spec(destination_chain_id);

    let config_call = ConfigContract::votingProtocolConfigCall { proposal_id };
    let config_returns = Contract::new(config_contract, &env)
        .call_builder(&config_call)
        .call();
    println!("Config Returns: {:?}", config_returns._0);

    let config =
        serde_json::from_str::<strategies::RiscVotingProtocolConfig>(&config_returns._0).unwrap();

    let strategies_context = strategies::Context::default(env);

    // Get the total voting power of the voter across all assets.
    let total_voting_power: U256 = config
        .assets
        .iter()
        .map(|asset| {
            // Get the accounts whost voting power is delegated to the voter.
            let delegations = strategies_context.process_delegation_strategy(
                voter,
                asset,
                Bytes::from_str(additional_delegation_data.as_str()).unwrap(),
            );
            if delegations.is_err() {
                println!("Delegations given are not correct");
                assert!(false);
            }
            delegations
                .unwrap()
                .iter()
                .fold(U256::from(0), |acc, delegation| {
                    (strategies_context.process_voting_strategy(
                        asset.voting_power_strategy.clone(),
                        delegation.delegate,
                        asset,
                    ) / delegation.ratio)
                        + acc
                })

            // assert_eq!(asset.chain_id, destination_chain_id.chain_id());
        })
        .sum::<U256>();

    println!("Total voting power: {}", total_voting_power);

    // General settings constraints
    assert!(direction == 1 || direction == 2 || direction == 3);

    assert!(balance > U256::from(0));
    println!(
        "Voter: {:?}, Signature Address: {:?}",
        voter,
        to_hex_string(&signature_address)
    );
    // assert!(voter.to_string() == to_hex_string(signature_address));
    assert!(balance == total_voting_power);

    // Commit the block hash and number used when deriving `view_call_env` to the journal.
    let journal = Journal {
        commitment: strategies_context.block_commitment(),
        config_contract,
        proposal_id,
        voter,
        balance,
        direction,
    };
    env::commit_slice(&journal.abi_encode());
}
