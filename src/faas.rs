use log::{error, info};
use solana_sdk::transaction::Transaction;
use std::error::Error;

// List of KYT addresses (public keys)
const KYT_ADDRESSES: [&str; 3] = [
    "ASXUgjoF6DKgYC6ifmjHt4UNWk1PvTjrtrCfQMbK3Y1o",
    "7nK8S6Hb9D9Qy6A7ogRjF5DdV8JxS8hY2u9W8HkK6x5Y",
    "B9M5U7xL3uM7G4n8jF4C8Y6P1vM5V7T8gF8K2xH8y3Y4",
];

pub struct Faas {}

impl Faas {
    pub async fn call_service(message: &str, raw_tx: &[u8]) -> Result<(), Box<dyn Error>> {
        let tx: Transaction = bincode::deserialize(&raw_tx).unwrap();
        match message {
            "KYT" => kyt(&tx),
            "KYC" => kyc(&tx),
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
