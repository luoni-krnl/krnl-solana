use log::{error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use solana_sdk::transaction::Transaction;
use std::env;
use std::error::Error;

// List of KYT addresses (public keys)
const KYT_ADDRESSES: [&str; 3] = [
    "ASXUgjoF6DKgYC6ifmjHt4UNWk1PvTjrtrCfQMbK3Y1o",
    "7nK8S6Hb9D9Qy6A7ogRjF5DdV8JxS8hY2u9W8HkK6x5Y",
    "B9M5U7xL3uM7G4n8jF4C8Y6P1vM5V7T8gF8K2xH8y3Y4",
];

#[derive(Serialize)]
struct ChatGptRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
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

pub struct Faas {}

impl Faas {
    pub async fn call_service(message: &str, raw_tx: &[u8]) -> Result<(), Box<dyn Error>> {
        let tx: Transaction = bincode::deserialize(&raw_tx).unwrap();
        match message {
            "KYT" => kyt(&tx),
            "KYC" => kyc(&tx),
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

    let sender = sender_pubkey.to_string();

    for &kyt_address in KYT_ADDRESSES.iter() {
        if kyt_address == sender {
            info!("KYT success for address: {}", sender);
            return Ok(());
        }
    }

    error!("KYT failed for address {}", sender);
    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("KYT failed for address {}", sender),
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
        messages: vec![Message {
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
