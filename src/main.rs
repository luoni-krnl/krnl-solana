use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use log::info;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct RequestBody {
    id: String,
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
}

#[post("/")]
async fn proxy(req_body: web::Json<Value>, client: web::Data<Arc<Client>>) -> impl Responder {
    let solana_url = "http://127.0.0.1:8899";

    let body_json = req_body.into_inner();
    let body: RequestBody = match serde_json::from_value(body_json) {
        Ok(body) => body,
        Err(err) => {
            let error_message = format!("Failed to deserialize request body: {}", err);
            info!("{}", error_message);
            return HttpResponse::BadRequest().body(error_message);
        }
    };

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
