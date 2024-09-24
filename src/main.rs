use clap::Parser;
use cosmrs::distribution::MsgWithdrawValidatorCommission;
use cosmrs::proto::prost::Message;
use cosmrs::tx::Msg;
use cosmrs::{
    crypto::secp256k1::SigningKey,
    rpc::Client,
    tendermint::{block::Height, chain::Id},
    tx::{AuthInfo, Body, Fee, SignDoc, SignerInfo},
    Coin,
};
use eyre::Result;
use std::{fs, str::FromStr};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "sommelier-3")]
    chain_id: String,

    #[arg(long)]
    signing_key_path: String,

    #[arg(long, default_value = "https://sommelier-rpc.polkachu.com:443")]
    rpc_url: String,

    #[arg(long, default_value = "https://sommelier-grpc.polkachu.com:14190")]
    grpc_url: String,

    #[arg(long, default_value = "usomm")]
    denom: String,

    #[arg(long, default_value = "0")]
    timeout_height: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Configure logging for stdout
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    log::info!("Starting withdraw-commission");
    let args = Args::parse();

    // Read private key from file
    let private_key = match fs::read_to_string(&args.signing_key_path) {
        Ok(key) => key.trim().to_string(),
        Err(e) => {
            log::error!("Failed to read private key from file: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to read private key from file: {}",
                e
            )));
        }
    };

    // Create the signing key from the private key
    let decoded_private_key = match hex::decode(&private_key) {
        Ok(decoded) => decoded,
        Err(e) => {
            log::error!("Failed to decode private key: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to decode private key: {}",
                e
            )));
        }
    };
    let signing_key = match SigningKey::from_slice(&decoded_private_key) {
        Ok(key) => key,
        Err(e) => {
            log::error!("Failed to create signing key: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to create signing key: {}",
                e
            )));
        }
    };

    // Derive the validator address from the private key
    let validator_address = match signing_key.public_key().account_id("somm") {
        Ok(validator_address) => validator_address,
        Err(e) => {
            log::error!("Failed to get validator address: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to get validator address: {}",
                e
            )));
        }
    };
    let validator_operator_address = match signing_key.public_key().account_id("sommvaloper") {
        Ok(validator_operator_address) => validator_operator_address,
        Err(e) => {
            log::error!("Failed to get validator operator address: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to get validator operator address: {}",
                e
            )));
        }
    };

    // log addresses
    log::info!("Validator address: {}", validator_address);
    log::info!("Validator operator address: {}", validator_operator_address);

    // Create the message
    let msg = MsgWithdrawValidatorCommission {
        validator_address: validator_operator_address,
    };

    // Create the transaction body
    let any = match msg.to_any() {
        Ok(any) => any,
        Err(e) => {
            log::error!("Failed to create any: {}", e);
            return Err(eyre::Report::msg(format!("Failed to create any: {}", e)));
        }
    };
    let tx_body = Body::new(
        vec![any],
        "Withdraw validator commission",
        Height::try_from(args.timeout_height)?,
    );

    // Set up the fee (adjust as needed)
    let coin = match Coin::new(1000, &args.denom) {
        Ok(coin) => coin,
        Err(e) => {
            log::error!("Failed to create coin: {}", e);
            return Err(eyre::Report::msg(format!("Failed to create coin: {}", e)));
        }
    };
    let fee = Fee::from_amount_and_gas(coin, 200000u64);

    // Create a client
    let channel = tonic::transport::Channel::from_shared(args.grpc_url.clone())?
        .connect()
        .await?;
    let mut query_client =
        cosmrs::proto::cosmos::auth::v1beta1::query_client::QueryClient::new(channel);
    let request = tonic::Request::new(cosmrs::proto::cosmos::auth::v1beta1::QueryAccountRequest {
        address: validator_address.to_string(),
    });
    let account_info = match query_client.account(request).await {
        Ok(account_info) => account_info,
        Err(e) => {
            log::error!("Failed to query account info: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to query account info: {}",
                e
            )));
        }
    };

    // Query the account information
    let account_any = account_info.into_inner().account.unwrap();
    let base_account = match cosmrs::proto::cosmos::auth::v1beta1::BaseAccount::decode(
        account_any.value.as_slice(),
    ) {
        Ok(base_account) => base_account,
        Err(e) => {
            log::error!("Failed to decode BaseAccount: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to decode BaseAccount: {}",
                e
            )));
        }
    };
    let account_number = base_account.account_number;
    let sequence_number = base_account.sequence;

    // Create the sign doc
    let chain_id = match Id::from_str(&args.chain_id) {
        Ok(chain_id) => chain_id,
        Err(e) => {
            log::error!("Failed to parse chain ID: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to parse chain ID: {}",
                e
            )));
        }
    };

    // Set up the signer info
    let signer_info = SignerInfo::single_direct(Some(signing_key.public_key()), sequence_number);
    let sign_doc = match SignDoc::new(
        &tx_body,
        &AuthInfo {
            fee,
            signer_infos: vec![signer_info],
        },
        &chain_id,
        account_number,
    ) {
        Ok(sign_doc) => sign_doc,
        Err(e) => {
            log::error!("Failed to create sign doc: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to create sign doc: {}",
                e
            )));
        }
    };

    // Sign the transaction
    let tx_raw = match sign_doc.sign(&signing_key) {
        Ok(tx_raw) => tx_raw,
        Err(e) => {
            log::error!("Failed to sign transaction: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to sign transaction: {}",
                e
            )));
        }
    };

    // Create a client and broadcast the transaction
    let Ok(client) = cosmrs::rpc::HttpClient::new(args.rpc_url.as_str()) else {
        log::error!("Failed to create client");
        return Err(eyre::Report::msg("Failed to create client"));
    };
    let tx_bytes = match tx_raw.to_bytes() {
        Ok(tx_bytes) => tx_bytes,
        Err(e) => {
            log::error!("Failed to convert transaction to bytes: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to convert transaction to bytes: {}",
                e
            )));
        }
    };
    let response = match client.broadcast_tx_commit(tx_bytes).await {
        Ok(response) => response,
        Err(e) => {
            log::error!("Failed to broadcast transaction: {}", e);
            return Err(eyre::Report::msg(format!(
                "Failed to broadcast transaction: {}",
                e
            )));
        }
    };

    println!("Response: {:?}", response);

    Ok(())
}
