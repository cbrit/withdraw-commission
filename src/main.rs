use clap::Parser;
use cosmrs::distribution::MsgWithdrawValidatorCommission;
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
    let Ok(private_key) = fs::read_to_string(&args.signing_key_path) else {
        log::error!("Failed to read private key from file");
        return Err(eyre::Report::msg("Failed to read private key from file"));
    };
    let private_key = private_key.trim();

    // Create the signing key from the private key
    let Ok(decoded_private_key) = hex::decode(private_key) else {
        log::error!("Failed to decode private key");
        return Err(eyre::Report::msg("Failed to decode private key"));
    };
    let Ok(signing_key) = SigningKey::from_slice(&decoded_private_key) else {
        log::error!("Failed to create signing key");
        return Err(eyre::Report::msg("Failed to create signing key"));
    };

    // Derive the validator address from the private key
    let Ok(validator_address) = signing_key.public_key().account_id("somm") else {
        log::error!("Failed to get validator address");
        return Err(eyre::Report::msg("Failed to get validator address"));
    };
    let Ok(validator_operator_address) = signing_key.public_key().account_id("sommvaloper") else {
        log::error!("Failed to get validator operator address");
        return Err(eyre::Report::msg(
            "Failed to get validator operator address",
        ));
    };

    // log addresses
    log::info!("Validator address: {}", validator_address);
    log::info!("Validator operator address: {}", validator_operator_address);

    // Create the message
    let msg = MsgWithdrawValidatorCommission {
        validator_address: validator_operator_address,
    };

    // Create the transaction body
    let Ok(any) = msg.to_any() else {
        log::error!("Failed to create any");
        return Err(eyre::Report::msg("Failed to create any"));
    };
    let tx_body = Body::new(
        vec![any],
        "Withdraw validator commission",
        Height::try_from(args.timeout_height)?,
    );

    // Set up the fee (adjust as needed)
    let Ok(coin) = Coin::new(1000, &args.denom) else {
        log::error!("Failed to parse coin");
        return Err(eyre::Report::msg("Failed to parse coin"));
    };
    let fee = Fee::from_amount_and_gas(coin, 200000u64);

    // Create a client
    let channel = tonic::transport::Channel::from_shared(args.grpc_url.clone())?
        .connect()
        .await?;
    let mut query_client =
        cosmrs::proto::cosmos::auth::v1beta1::query_client::QueryClient::new(channel);
    let request = tonic::Request::new(
        cosmrs::proto::cosmos::auth::v1beta1::QueryAccountInfoRequest {
            address: validator_address.to_string(),
        },
    );
    let account_info = match query_client.account_info(request).await {
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
    let account_number = account_info.into_inner().info.unwrap().account_number;

    // Create the sign doc
    let Ok(chain_id) = Id::from_str(&args.chain_id) else {
        log::error!("Failed to parse chain ID");
        return Err(eyre::Report::msg("Failed to parse chain ID"));
    };

    // Set up the signer info
    let signer_info = SignerInfo::single_direct(Some(signing_key.public_key()), 0);
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

    log::info!("Transaction submitted successfully!");
    log::info!("Transaction hash: {}", response.hash);

    Ok(())
}
