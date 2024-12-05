use alloy::{
    primitives::{keccak256, Address, FixedBytes, U256},
    signers::{local::PrivateKeySigner, Signature, Signer},
};
use anyhow::Result;

const PREFIX: &str = "\x19Ethereum Signed Message:\n32";

fn hash_vote(
    chain_id: u64,
    dao_address: Address,
    proposal_id: U256,
    direction: u8,
    balance: U256,
) -> FixedBytes<32> {
    let concat_data = [
        chain_id.to_be_bytes().to_vec(),
        dao_address.to_vec(),
        proposal_id.to_be_bytes_vec(),
        [direction].to_vec(),
        balance.to_be_bytes_vec(),
    ]
    .concat();

    let hashed_data = keccak256(&concat_data).to_vec();
    keccak256([PREFIX.as_bytes(), &hashed_data].concat())
}

pub async fn get_user_vote_signature(
    chain_id: u64,
    signer: PrivateKeySigner,
    dao_address: Address,
    proposal_id: U256,
    direction: u8,
    balance: U256,
) -> Result<Signature> {
    let vote_hash = hash_vote(chain_id, dao_address, proposal_id, direction, balance);

    let signature = signer
        .with_chain_id(Some(chain_id))
        .sign_hash(&vote_hash)
        .await?;

    Ok(signature)
}
