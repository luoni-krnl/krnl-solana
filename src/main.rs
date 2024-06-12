use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use base64::{decode, encode};
use log::info;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::error::Error;
use std::str;
use std::string::String;
use std::sync::Arc;

mod faas;

use crate::faas::Faas;

#[derive(Serialize, Deserialize)]
pub struct RequestBody {
    id: String,
    jsonrpc: String,
    method: String,
    params: Vec<Value>,
}

#[derive(Serialize, Deserialize)]
struct TxRequest {
    accessToken: String,
    message: String,
}

struct FaasMessage {
    messages: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct SignatureToken {
    signatureToken: String,
    hash: String,
}

async fn call_token_authority(path: &str, payload: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let token_authority = "http://127.0.0.1:8181";
    let url = format!("{}{}", token_authority, path);

    let client = Client::new();
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(payload.to_vec())
        .send()
        .await?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("transaction rejected: invalid access token".into());
    }

    if resp.status() == reqwest::StatusCode::BAD_REQUEST {
        return Err("transaction rejected: no FaaS request specified".into());
    }

    let body = resp.bytes().await?;
    Ok(body.to_vec())
}

#[post("/")]
async fn proxy(req_body: web::Json<Value>, client: web::Data<Arc<Client>>) -> impl Responder {
    let solana_url = "http://127.0.0.1:8899";

    let body_json = req_body.into_inner();
    let mut body: RequestBody = match serde_json::from_value(body_json) {
        Ok(body) => body,
        Err(err) => {
            let error_message = format!("Failed to deserialize request body: {}", err);
            info!("{}", error_message);
            return HttpResponse::BadRequest().body(error_message);
        }
    };

    info!("Request Method: {:?}", body.method);

    if body.method == "krnl_transactionRequest" {
        let tx_request: Vec<TxRequest> =
            match serde_json::from_value(serde_json::Value::Array(body.params.clone())) {
                Ok(tx_request) => tx_request,
                Err(err) => {
                    let error_message = format!("Failed to deserialize TxRequest: {}", err);
                    info!("{}", error_message);
                    return HttpResponse::BadRequest().body(error_message);
                }
            };

        let tx_request_payload = match serde_json::to_vec(&tx_request[0]) {
            Ok(payload) => payload,
            Err(err) => {
                let error_message = format!("Failed to serialize TxRequest: {}", err);
                info!("{}", error_message);
                return HttpResponse::InternalServerError().body(error_message);
            }
        };

        let body = match call_token_authority("/tx-request", &tx_request_payload).await {
            Ok(body) => body,
            Err(err) => {
                let error_message = format!("Token Authority error: {}", err);
                info!("{}", error_message);
                return HttpResponse::InternalServerError().body(error_message);
            }
        };

        let signature_token: SignatureToken = match serde_json::from_slice(&body) {
            Ok(token) => token,
            Err(err) => {
                let error_message = format!("Failed to deserialize SignatureToken: {}", err);
                info!("{}", error_message);
                return HttpResponse::InternalServerError().body(error_message);
            }
        };

        return HttpResponse::Ok().json(signature_token);
    }

    if body.method == "sendTransaction" {
        let tx = match body.params[0].as_str() {
            Some(tx) => tx,
            None => {
                let error_message = "First parameter is not a string".to_string();
                info!("{}", error_message);
                return HttpResponse::BadRequest().body(error_message);
            }
        };
        let decoded_data = match decode(tx) {
            Ok(data) => data,
            Err(_) => {
                let error_message = "Failed to decode base64".to_string();
                info!("{}", error_message);
                return HttpResponse::BadRequest().body(error_message);
            }
        };

        let separator = b':';

        if let Some(pos) = decoded_data.iter().position(|&byte| byte == separator) {
            let fass_request_part = &decoded_data[pos + 1..];
            let tx_part = &decoded_data[..pos];
            let encoded_tx_part = encode(tx_part);

            if let Some(param_mut) = body.params.get_mut(0) {
                *param_mut = Value::String(encoded_tx_part);
            }

            if let Ok(fass_request_str) = str::from_utf8(fass_request_part) {
                let messages: Vec<String> =
                    fass_request_str.split(':').map(|s| s.to_string()).collect();

                let fass_request = FaasMessage { messages };
                for message in fass_request.messages.iter() {
                    info!("faas message: {:?}", message);
                    if let Err(err) = Faas::call_service(message, tx_part, &client).await {
                        let error_message = format!("Error: {}", err);
                        info!("{}", error_message);
                        return HttpResponse::InternalServerError().body(error_message);
                    };
                }
            } else {
                let error_message = "Failed to convert bytes to string".to_string();
                info!("{}", error_message);
                return HttpResponse::BadRequest().body(error_message);
            }
        }
    }

    let response = client.post(solana_url).json(&body).send().await;

    match response {
        Ok(res) => {
            let status = res.status();
            let body = res
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read response body".to_string());
            info!("Response status: {}, body: {}", status, body);
            HttpResponse::build(status).body(body)
        }
        Err(err) => {
            let error_message = format!("Error: {}", err);
            info!("{}", error_message);
            HttpResponse::InternalServerError().body(error_message)
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // let client = Arc::new(RpcClient::new("http://127.0.0.1:8899".to_string()));

    let client = Arc::new(Client::new());

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(client.clone()))
            .service(proxy)
    })
    .bind("127.0.0.1:8999")?
    .run()
    .await
}
