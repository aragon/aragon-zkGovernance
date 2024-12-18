use std::str::FromStr;

use alloy::{
    network::EthereumWallet, providers::ProviderBuilder, signers::local::PrivateKeySigner,
    sol_types::SolValue, transports::http::reqwest::Url,
};
use alloy_primitives::{Address, U256};
use anyhow::{ensure, Context, Result};
use apps::HostContext;
use aragon_zk_voting_protocol_methods::EXECUTION_PROTOCOL_ELF;
use clap::Parser;
use risc0_ethereum_contracts::encode_seal;
use risc0_steel::{
    ethereum::{EthEvmEnv, ETH_SEPOLIA_CHAIN_SPEC},
    Commitment, Contract,
};
use risc0_zkvm::{default_prover, ExecutorEnv, ProveInfo, ProverOpts, VerifierContext};
use tokio::task;
use tracing_subscriber::EnvFilter;

alloy::sol! {
    interface ConfigContract {
        function votingProtocolConfig(uint256 proposal_id) external view returns (string memory);
    }

    // TODO: Journal should be in sync with guest
    struct VotingJournal {
        Commitment commitment;
        address configContract;
        uint256 proposalId;
        address voter;
        uint256 balance;
        uint8 direction;
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
    /// Account address of the DAO the voter is voting for
    #[clap(long)]
    dao_address: Address,

    /// Proposal ID
    #[clap(long)]
    proposal_id: U256,

    /// Plugin's contract address on Ethereum
    #[clap(long)]
    config_contract: Address,

    /// Live tally of the proposal
    #[clap(short, long, value_delimiter = ' ', num_args = 3)]
    tally: Vec<U256>,

    // If proving should be disabled
    #[clap(long)]
    testing: u8,
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

    let tally: [U256; 3] = args
        .tally
        .try_into()
        .expect("Expected tally input to be an array of 3");

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
        .rpc(Url::from_str(&args.rpc_url).unwrap())
        // .beacon_api(Url::from_str(&args.rpc_url).unwrap())
        // .provider(provider.clone())
        .block_number(args.block_number.unwrap())
        .build()
        .await?;

    //  The `with_chain_spec` method is used to specify the chain configuration.
    env = env.with_chain_spec(&ETH_SEPOLIA_CHAIN_SPEC);

    // Making the preflighs. This step is mandatory
    let config_call = ConfigContract::votingProtocolConfigCall {
        proposal_id: args.proposal_id,
    };
    let mut config_contract = Contract::preflight(args.config_contract, &mut env);
    let config_returns = config_contract.call_builder(&config_call).call().await?;
    println!("Config string: {:?}", config_returns._0);

    let config =
        serde_json::from_str::<apps::RiscVotingProtocolConfig>(&config_returns._0).unwrap();

    let mut strategies_context = HostContext::default(&mut env);

    // Get the total voting power of the voter across all assets.
    let mut total_voting_power = U256::from(0);

    for asset in &config.assets {
        // Call the async function and await the result
        let voting_power = strategies_context.process_total_supply(asset).await?;

        total_voting_power += voting_power;
    }

    assert!(
        strategies_context
            .process_execution_strategy(config.execution_strategy, total_voting_power, tally)
            .await?
    );

    println!("Total voting power: {}", total_voting_power);
    println!("proving...");

    if args.testing == 1 {
        return Ok(());
    }

    let view_call_input = env.into_input().await?;
    let prove_info = task::spawn_blocking(move || -> Result<ProveInfo, anyhow::Error> {
        let env = ExecutorEnv::builder()
            .write(&view_call_input)?
            .write(&args.dao_address)?
            .write(&args.proposal_id)?
            .write(&args.config_contract)?
            .write(&tally)?
            .build()?;

        default_prover().prove_with_ctx(
            env,
            &VerifierContext::default(),
            EXECUTION_PROTOCOL_ELF,
            &ProverOpts::groth16(),
        )
    })
    .await?
    .context("failed to create proof")?;
    println!("proving...done");

    // Encode the groth16 seal with the selector
    let receipt = prove_info.receipt;
    let journal = &receipt.journal.bytes;

    // Decode and log the commitment
    let journal = VotingJournal::abi_decode(journal, true).context("invalid journal")?;

    // ABI encode the seal.
    let seal = encode_seal(&receipt).context("invalid receipt")?;
    let seal_bytes = seal.as_slice();

    // println!("journalData: {:?}", to_hex_string(journal));
    println!("seal: {:?}", to_hex_string(seal_bytes));
    println!("Steel commitment: {:?}", journal.commitment);

    let contract = IMajorityVoting::new(args.config_contract, &provider);
    let call_builder = contract.execute(receipt.journal.bytes.into(), seal.into());
    println!("Send {} {}", contract.address(), call_builder.calldata());
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
