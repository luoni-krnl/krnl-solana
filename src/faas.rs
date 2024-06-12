use actix_web::web;
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use log::{error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use solana_sdk::{bs58, pubkey, pubkey::Pubkey, transaction::Transaction};
use std::env;
use std::error::Error;
use std::sync::Arc;

// List of KYT addresses (public keys)
const KYT_ADDRESSES: [Pubkey; 3] = [
    pubkey!("9LGvtfGz78yuxAYwbwapg3tD7ZVZmeYkhSBuyW7Q6eEN"),
    pubkey!("7nK8S6Hb9D9Qy6A7ogRjF5DdV8JxS8hY2u9W8HkK6x5Y"),
    pubkey!("B9M5U7xL3uM7G4n8jF4C8Y6P1vM5V7T8gF8K2xH8y3Y4"),
];

// Policy Engine address
const POLICY_ENGINE: Pubkey = pubkey!("9TRhu4fGB2nPXFGWQUj9sLZdte5bpPucNVcAgLeKXE96");
const ALLOW_LIST: Pubkey = pubkey!("DdyeuHFukFCEUo6tTavwF81vqcvSg5gooKTgcKd9kc2B");

#[derive(Serialize)]
struct ChatGptRequest {
    model: String,
    messages: Vec<ChatGptMessage>,
}

#[derive(Serialize)]
struct ChatGptMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatGptResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

#[derive(Serialize, Deserialize)]
pub struct RequestBody {
    id: String,
    jsonrpc: String,
    method: String,
    params: Vec<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct AccountInfoResponse {
    result: Option<AccountInfoResult>,
    error: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct AccountInfoResult {
    context: Context,
    value: AccountInfo,
}

#[derive(Serialize, Deserialize, Debug)]
struct Context {
    slot: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct AccountInfo {
    data: Vec<String>,
    owner: String,
    lamports: u64,
}

// Define the AllowList struct for Borsh deserialization
#[derive(BorshDeserialize, BorshSerialize, BorshSchema, Debug)]
struct AllowList {
    discriminator: [u8; 8],
    length: u32,
    allowed_receivers: [[u8; 32]; 10],
}

pub struct Faas {}

impl Faas {
    pub async fn call_service(
        message: &str,
        raw_tx: &[u8],
        client: &web::Data<Arc<Client>>,
    ) -> Result<(), Box<dyn Error>> {
        let tx: Transaction = bincode::deserialize(&raw_tx).unwrap();
        match message {
            "KYT" => kyt(&tx),
            "KYC" => kyc(&tx),
            "PE" => policy_engine(&tx, client).await,
            s if s.contains("GPT") => chat_gpt(s).await,
            _ => {
                let error_message = format!("Unknown function name: {}", message);
                error!("{}", error_message);
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    error_message,
                )))
            }
        }
    }
}

fn kyt(transaction: &Transaction) -> Result<(), Box<dyn Error>> {
    // Get the sender's public key from the transaction
    let sender_pubkey = transaction.message.account_keys[0];

    for &kyt_address in KYT_ADDRESSES.iter() {
        if kyt_address == sender_pubkey {
            info!("KYT success for address: {}", sender_pubkey);
            return Ok(());
        }
    }

    error!("KYT failed for address {}", sender_pubkey);
    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("KYT failed for address {}", sender_pubkey),
    )))
}

fn kyc(transaction: &Transaction) -> Result<(), Box<dyn Error>> {
    info!("KYC FaaS not implemented");
    return Ok(());
}

async fn chat_gpt(query: &str) -> Result<(), Box<dyn Error>> {
    let api_key = env::var("OPENAI_KEY")?;

    let client = Client::new();
    let url = "https://api.openai.com/v1/chat/completions";
    let request_body = ChatGptRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![ChatGptMessage {
            role: "user".to_string(),
            content: format!("{} You must reply Yes or No", query),
        }],
    };

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        error!("Failed to get response from ChatGPT API: {:?}", response);
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to get response from ChatGPT API",
        )));
    }

    let response_body: ChatGptResponse = response.json().await?;

    let answer = response_body.choices[0].message.content.to_lowercase();
    info!("ChatGPT answer: {}", answer);

    if answer.contains("yes") {
        info!("ChatGPT FaaS success");
        Ok(())
    } else {
        error!("ChatGPT FaaS denied transaction");
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "ChatGPT FaaS denied transaction",
        )))
    }
}

async fn policy_engine(
    tx: &Transaction,
    client: &web::Data<Arc<Client>>,
) -> Result<(), Box<dyn Error>> {
    let sender_pubkey = tx.message.account_keys[0];

    // Serialize and encode the transaction
    let pubkey_base58 = bs58::encode(ALLOW_LIST.to_bytes()).into_string();
    let get_account_info_json = RequestBody {
        jsonrpc: "2.0".to_string(),
        id: "1".to_string(),
        method: "getAccountInfo".to_string(),
        params: vec![
            serde_json::Value::String(pubkey_base58),
            json!({"encoding": "base64"}),
        ],
    };

    match client
        .post("http://127.0.0.1:8899".to_string())
        .json(&get_account_info_json)
        .send()
        .await
    {
        Ok(res) => {
            let status = res.status();
            let body = res
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read response body".to_string());
            info!("Response status: {}, body: {}", status, body);
            match serde_json::from_str::<AccountInfoResponse>(&body) {
                Ok(parsed_body) => {
                    if let Some(account_data) = parsed_body.result {
                        if let Some(encoded_data) = account_data.value.data.get(0) {
                            let decoded_data = base64::decode(encoded_data)?;
                            let allow_list: AllowList =
                                BorshDeserialize::try_from_slice(&decoded_data)?;

                            let mut allowed = false;
                            for receiver in allow_list.allowed_receivers {
                                let pubkey = Pubkey::new(&receiver);
                                if pubkey == sender_pubkey {
                                    info!("Tx allowed by Policy Engine");
                                    allowed = true;
                                    break;
                                }
                            }

                            if allowed == false {
                                let error_message =
                                    format!("policy engine - unrecognised receiver");
                                info!("{}", error_message);
                                return Err(Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    error_message,
                                )));
                            }
                        }
                    }
                }
                Err(e) => {
                    info!("Failed to decode response: {}", e);
                }
            }
            Ok(())
        }
        Err(err) => {
            let error_message = format!("Error: {}", err);
            info!("{}", error_message);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                error_message,
            )));
        }
    }
}
