use std::str::FromStr;

use alloy::{
    hex,
    network::{Ethereum, EthereumWallet},
    node_bindings::{Anvil, AnvilInstance},
    primitives::{address, bytes, keccak256, Address, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    transports::http::Http,
};
use alloy_sol_types::{sol, SolValue};
use anyhow::Result;
use rand::Rng;
use std::process::Command;
use tests::get_user_vote_signature;

sol!(
    #[sol(rpc)]
    MockVerifier,
    "../out/RiscZeroMockVerifier.sol/RiscZeroMockVerifier.json"
);

sol!(
    #[sol(rpc)]
    RiscVotingProtocolPlugin,
    "../out/RiscVotingProtocolPlugin.sol/RiscVotingProtocolPlugin.json"
);

sol!(
    #[sol(rpc)]
    RiscVotingProtocolPluginSetup,
    "../out/RiscVotingProtocolPluginSetup.sol/RiscVotingProtocolPluginSetup.json"
);

sol!(
    #[sol(rpc)]
    interface PluginRepoFactory {
        function createPluginRepoWithFirstVersion(
            string calldata _subdomain,
            address _pluginSetup,
            address _maintainer,
            bytes memory _releaseMetadata,
            bytes memory _buildMetadata
        ) external returns (address);
    }

    #[sol(rpc)]
    interface DAOFactory {
        function createDao(
            DAOSettings calldata _daoSettings,
            PluginSettings[] calldata _pluginSettings
        ) external returns (address);
    }

    struct DAOSettings {
        address trustedForwarder;
        string daoURI;
        string subdomain;
        bytes metadata;
    }

    enum VotingMode {
        Standard,
        EarlyExecution,
        VoteReplacement
    }

    struct VotingSettings {
        VotingMode votingMode;
        uint32 supportThreshold;
        uint32 minParticipation;
        uint64 minDuration;
        uint256 minProposerVotingPower;
        string votingProtocolConfig;
    }

    struct Tag {
        uint8 release;
        uint16 build;
    }

    struct PluginSetupRef {
        Tag versionTag;
        address pluginSetupRepo;
    }
    struct PluginSettings {
        PluginSetupRef pluginSetupRef;
        bytes data;
    }

    struct PluginSettingsData {
        VotingSettings votingSettings;
        address token;
        address verifier;
    }
);

async fn setup_test_environment() -> Result<(
    AnvilInstance,
    impl Provider<Http<reqwest::Client>, Ethereum>,
    Address,
)> {
    // Set dev mode for test.
    std::env::set_var("RISC0_DEV_MODE", "true");
    let sepolia_rpc_url = std::env::var("RPC_URL").expect("RPC_URL env not set");

    // Spin up a local Anvil node.
    let anvil = Anvil::new().fork(sepolia_rpc_url).try_spawn()?;

    // Set up signer from the first default Anvil account (Alice).
    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let wallet = EthereumWallet::from(signer);
    println!("Wallet address: {:?}", wallet.default_signer().address());

    // Create a provider with the wallet.
    let rpc_url = anvil.endpoint().parse()?;

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url);

    let verifier = MockVerifier::deploy(&provider, [0, 0, 0, 0].into())
        .await?
        .address()
        .clone();

    Ok((anvil, provider, verifier))
}

async fn dao_setup(
    provider: &impl Provider<Http<reqwest::Client>, Ethereum>,
    verifier: Address,
) -> anyhow::Result<(Address, Address, Address, Address)> {
    let account = provider.get_accounts().await?[0];
    let mut rng = rand::thread_rng();

    let n2: u16 = rng.gen();
    let name_with_entropy = format!("risc-zero-plugin-{}", n2);

    // 1. Deploying the Plugin Setup contract.
    let plugin_setup = RiscVotingProtocolPluginSetup::deploy(provider).await?;

    // 2. Publishing it in the Aragon OSx Protocol.
    let plugin_repo_contract = PluginRepoFactory::new(
        address!("07f49c49Ce2A99CF7C28F66673d406386BDD8Ff4"),
        provider,
    );

    let plugin_repo = plugin_repo_contract
        .createPluginRepoWithFirstVersion(
            name_with_entropy.clone(),
            *plugin_setup.address(),
            account,
            bytes!("00"),
            bytes!("00"),
        )
        .call()
        .await?
        ._0;
    let _plugin_repo_send = plugin_repo_contract
        .createPluginRepoWithFirstVersion(
            name_with_entropy.clone(),
            *plugin_setup.address(),
            account,
            bytes!("00"),
            bytes!("00"),
        )
        .send()
        .await?
        .get_receipt()
        .await?;

    // 3. Get the DAO Settings.
    let dao_settings = DAOSettings {
        trustedForwarder: Address::ZERO,
        daoURI: "".to_string(),
        subdomain: name_with_entropy,
        metadata: bytes!("00"),
    };

    // 5. Get the plugin settings.
    let voting_protocol_config = r#"{"votingProtocolVersion":"1","assets":[{"contract":"0x185Bb1cca668C474214e934028A3e4BB7A5E6525","chainId":11155111,"votingPowerStrategy":"BalanceOf","delegation":{"contract":"0x32Bb2dB7826cf342743fe80832Fe4DF725879C2D","strategy":"SplitDelegation"}}],"executionStrategy":"MajorityVoting"}"#;

    let voting_settings = VotingSettings {
        votingMode: VotingMode::Standard,
        supportThreshold: 20,
        minParticipation: 10,
        minDuration: 3600, // 1 hour
        minProposerVotingPower: U256::from(0),
        votingProtocolConfig: voting_protocol_config.to_string(),
    };

    let encoded_plugin_settings_data = PluginSettingsData {
        votingSettings: voting_settings,
        token: address!("185Bb1cca668C474214e934028A3e4BB7A5E6525"),
        verifier,
    }
    .abi_encode_params();

    let plugin_settings = vec![PluginSettings {
        pluginSetupRef: PluginSetupRef {
            versionTag: Tag {
                release: 1,
                build: 1,
            },
            pluginSetupRepo: plugin_repo,
        },
        data: encoded_plugin_settings_data.into(),
    }];

    // 6. Deploy the DAO.
    let dao_contract = DAOFactory::new(
        address!("7a62da7B56fB3bfCdF70E900787010Bc4c9Ca42e"),
        provider,
    );

    let dao_call = dao_contract.createDao(dao_settings, plugin_settings);
    let dao = dao_call.call().await?._0;
    let dao_send = dao_call.send().await?;
    let mut plugin_address: Address = Default::default();

    // 7. Get the plugin address from the logs.
    dao_send
        .get_receipt()
        .await?
        .inner
        .logs()
        .iter()
        .for_each(|log| {
            let first_log = log.topic0().unwrap();
            if keccak256("InstallationApplied(address,address,bytes32,bytes32)") == *first_log {
                plugin_address = Address::from_slice(log.topics()[2][12..].as_ref());
            }
        });

    Ok((*plugin_setup.address(), plugin_repo, plugin_address, dao))
}

#[tokio::test]
async fn test_config_is_setup() -> Result<()> {
    println!("Setting up test environment");
    let (anvil, provider, verifier) = setup_test_environment().await?;
    let (plugin_setup, plugin_repo, plugin, dao) = dao_setup(&provider, verifier).await?;

    println!();
    println!("-----------------------------------------------------");
    println!("Risc Zero Verifier address: {:?}", verifier);
    println!("Plugin setup address: {:?}", plugin_setup);
    println!("Plugin repo address: {:?}", plugin_repo);
    println!("Plugin address: {:?}", plugin);
    println!("DAO address: {:?}", dao);
    println!("-----------------------------------------------------");
    println!();

    let plugin_contract = RiscVotingProtocolPlugin::new(plugin, &provider);
    plugin_contract
        .createProposal(bytes!("00"), vec![], U256::from(0), 0, 0)
        .send()
        .await?
        .get_receipt()
        .await?;

    println!("Proposal created: 0");

    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();

    // let balance = U256::from_str("100000000000000000").expect("Failed to parse balance");
    let balance = U256::from_str("0").expect("Failed to parse balance");

    let signed_vote = hex::encode(
        get_user_vote_signature(11155111, signer.clone(), dao, U256::from(2), 0, balance)
            .await?
            .as_bytes(),
    );

    let voter = EthereumWallet::from(signer).default_signer().address();
    let token = address!("185Bb1cca668C474214e934028A3e4BB7A5E6525");
    // let additional_delegation_data = "8bF1e340055c7dE62F11229A149d3A1918de3d74";
    let additional_delegation_data = "";

    println!("Running publisher");

    let output = Command::new("cargo")
        .current_dir("../")
        .env("BONSAI_API_KEY", std::env::var("BONSAI_API_KEY").unwrap())
        .env("BONSAI_API_URL", std::env::var("BONSAI_API_URL").unwrap())
        .env("RPC_URL", std::env::var("RPC_URL").unwrap())
        .env(
            "ETH_WALLET_PRIVATE_KEY",
            std::env::var("ETH_WALLET_PRIVATE_KEY").unwrap(),
        )
        .arg("run")
        .arg("--bin")
        .arg("publisher")
        .arg("--")
        .arg(format!(
            "--chain-id={}",
            std::env::var("CHAIN_ID").unwrap_or_else(|_| "11155111".to_string())
        ))
        .arg(format!("--rpc-url={}", anvil.endpoint()))
        .arg(format!(
            "--block-number={}",
            provider.get_block_number().await?
        ))
        .arg(format!("--voter-signature={:?}", signed_vote))
        .arg(format!("--voter={}", voter))
        .arg(format!("--dao-address={}", dao))
        .arg(format!("--proposal-id={}", 0))
        .arg(format!("--direction={}", 2))
        .arg(format!("--balance={}", balance))
        .arg(format!("--config-contract={}", plugin))
        .arg(format!("--token={}", token))
        .arg(format!("--testing={}", 1))
        .arg(format!(
            "--additional-delegation-data={}",
            additional_delegation_data
        ))
        .output()
        .expect("failed to execute process");

    println!("Execution done");
    let message_out = String::from_utf8_lossy(&output.stdout).to_string();
    let message_stderr = String::from_utf8_lossy(&output.stderr).to_string();
    println!("{}", message_out);
    println!("{}", message_stderr);

    Ok(())
}
