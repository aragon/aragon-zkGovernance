use std::str::FromStr;

use alloy::{
    network::EthereumWallet, providers::ProviderBuilder, signers::local::PrivateKeySigner,
    transports::http::reqwest::Url,
};
use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::sol;
use anyhow::{ensure, Context, Result};
use apps::HostContext;
use aragon_zk_voting_protocol_methods::VOTING_PROTOCOL_ELF;
use clap::Parser;
use risc0_ethereum_contracts::groth16::encode;
use risc0_steel::{
    ethereum::{EthEvmEnv, ETH_SEPOLIA_CHAIN_SPEC},
    host::BlockNumberOrTag,
    Contract,
};
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, VerifierContext};
use tracing_subscriber::EnvFilter;

sol! {
    /// ERC-20 balance function signature.
    /// This must match the signature in the guest.
    interface IERC20 {
        function balanceOf(address account) external view returns (uint);
    }
    interface ConfigContract {
        function getVotingProtocolConfig() external view returns (string memory);
    }
}

alloy::sol!(
    #[sol(rpc, all_derives)]
    "../contracts/IMajorityVoting.sol"
);

/// Arguments of the publisher CLI.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Ethereum chain ID
    #[clap(long)]
    chain_id: u64,

    /// Ethereum Node endpoint.
    #[clap(long, env)]
    eth_wallet_private_key: PrivateKeySigner,

    /// Ethereum Node endpoint.
    #[clap(long, env)]
    rpc_url: String,

    /// Ethereum block number.
    #[clap(long)]
    block_number: Option<u64>,

    /// Voter's signature
    #[clap(long)]
    voter_signature: String,

    /// Account address to read the balance_of on Ethereum
    #[clap(long)]
    voter: Address,

    /// Account address of the DAO the voter is voting for
    #[clap(long)]
    dao_address: Address,

    /// Proposal ID
    #[clap(long)]
    proposal_id: U256,

    /// Vote direction
    #[clap(long)]
    direction: u8,

    /// Voter's balance
    #[clap(long)]
    balance: U256,

    /// Plugin's contract address on Ethereum
    #[clap(long)]
    config_contract: Address,

    /// ERC20 contract address on Ethereum
    #[clap(long)]
    token: Address,

    /// Additional delegation data
    #[clap(long)]
    additional_delegation_data: String,
}

fn to_hex_string(bytes: &[u8]) -> String {
    // Convert each byte to its hexadecimal representation and collect into a single String
    bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing. In order to view logs, run `RUST_LOG=info cargo run`
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // parse the command line arguments
    let args = Args::parse();

    // Create an alloy provider for that private key and URL.
    let wallet = EthereumWallet::from(args.eth_wallet_private_key);
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(Url::from_str(&args.rpc_url).unwrap());

    // Create an EVM environment from an RPC endpoint and a block number. If no block number is
    // provided, the latest block is used.
    // Define the type for H
    let mut env = EthEvmEnv::builder()
        .provider(provider.clone())
        .block_number_or_tag(BlockNumberOrTag::Number(args.block_number.unwrap()))
        .build()
        .await?;

    //  The `with_chain_spec` method is used to specify the chain configuration.
    env = env.with_chain_spec(&ETH_SEPOLIA_CHAIN_SPEC);

    // Making the preflighs. This step is mandatory
    let config_call = ConfigContract::getVotingProtocolConfigCall {};
    let mut config_contract = Contract::preflight(args.config_contract, &mut env);
    let config_returns = config_contract.call_builder(&config_call).call().await?;
    println!("Config string: {:?}", config_returns._0);

    let config =
        serde_json::from_str::<apps::RiscVotingProtocolConfig>(&config_returns._0).unwrap();

    let mut strategies_context = HostContext::default(&mut env);

    // Get the total voting power of the voter across all assets.
    let total_voting_power: U256 = config
        .assets
        .iter()
        .map(|asset| {
            // Get the accounts whost voting power is delegated to the voter.
            let delegations = strategies_context.process_delegation_strategy(
                args.voter,
                asset,
                Bytes::from_str(args.additional_delegation_data.as_str()).unwrap(),
            );
            if delegations.is_err() {
                println!("Delegations given are not correct");
                assert!(false);
            }
            delegations
                .unwrap()
                .iter()
                .fold(U256::from(0), |acc, delegation| {
                    (strategies_context.process_voting_power_strategy(
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
    println!("proving...");

    let view_call_input = env.into_input().await?;
    let env = ExecutorEnv::builder()
        .write(&view_call_input)?
        .write(&args.voter_signature)?
        .write(&args.voter)?
        .write(&args.dao_address)?
        .write(&args.proposal_id)?
        .write(&args.direction)?
        .write(&args.balance)?
        .write(&args.config_contract)?
        .write(&args.additional_delegation_data)?
        .build()?;

    let receipt = default_prover()
        .prove_with_ctx(
            env,
            &VerifierContext::default(),
            VOTING_PROTOCOL_ELF,
            &ProverOpts::groth16(),
        )?
        .receipt;
    println!("proving...done");

    // Encode the groth16 seal with the selector
    let seal = encode(receipt.inner.groth16()?.seal.clone())?;
    let journal_bytes = receipt.journal.bytes.as_slice();
    let seal_bytes = seal.as_slice();

    println!("journalData: {:?}", to_hex_string(journal_bytes));
    println!("seal: {:?}", to_hex_string(seal_bytes));

    let contract = IMajorityVoting::new(args.config_contract, &provider);
    let call_builder = contract.vote(receipt.journal.bytes.into(), seal.into());
    log::debug!("Send {} {}", contract.address(), call_builder.calldata());
    let pending_tx = call_builder.send().await?;
    let tx_hash = *pending_tx.tx_hash();
    let receipt = pending_tx
        .get_receipt()
        .await
        .with_context(|| format!("transaction did not confirm: {}", tx_hash))?;

    ensure!(receipt.status(), "transaction failed: {}", tx_hash);

    println!("sending tx...done");

    Ok(())
}
