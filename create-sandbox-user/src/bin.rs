use anyhow::{bail, Result};
use http::StatusCode;
use reqwest::blocking;
use serde::Deserialize;
use serde_json::json;
use structopt::StructOpt;
use uuid::Uuid;

mod common;

use common::FALLBACK_CALLBACK_HOST;

#[derive(Debug)]
struct SandboxUser {
    username: String,
    password: String,
    callback_host: String,
}

#[derive(Debug, StructOpt)]
struct CliConfig {
    #[structopt(long = "callback-host")]
    callback_host: Option<String>,
    #[structopt(long = "subscription-key")]
    subscription_key: String,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct SandboxApiKey {
    apiKey: String,
}

fn create_sandbox_credentials(cli_config: &CliConfig) -> Result<SandboxUser> {
    let http_client = blocking::Client::builder()
        .http1_title_case_headers()
        .build()?;

    // callback_host must be the host for all callback urls used with
    // the to be created sandbox api user credentials
    let callback_host = if let Some(host) = &cli_config.callback_host {
        host
    } else {
        // there is a mock PUT endpoint @
        // https://www.mocky.io/v2/5ec0fa1c2f000079004c86fb
        println!(
            "[mini-mtn-momo] using fallback callback host {}",
            FALLBACK_CALLBACK_HOST
        );

        FALLBACK_CALLBACK_HOST
    };

    let user_id_string = Uuid::new_v4().to_string();

    let response = http_client
        .post("https://sandbox.momodeveloper.mtn.com/v1_0/apiuser")
        .header("Ocp-Apim-Subscription-Key", &cli_config.subscription_key)
        .header("X-Reference-Id", &user_id_string)
        .header("Content-Type", "application/json")
        .body(json!({ "providerCallbackHost": callback_host }).to_string())
        .send()?;

    if response.status() != StatusCode::CREATED {
        bail!(
            "creating a sandbox user failed - http status {:?}",
            response.status()
        );
    }

    let url = format!(
        "https://sandbox.momodeveloper.mtn.com/v1_0/apiuser/{}/apikey",
        &user_id_string,
    );

    let response = http_client
        .post(&url)
        .header("Ocp-Apim-Subscription-Key", &cli_config.subscription_key)
        .header("X-Reference-Id", &user_id_string)
        .header("Content-Length", "0")
        .send()?;

    if response.status() != StatusCode::CREATED {
        bail!(
            "creating a sandbox api key failed - http status {:?}",
            response.status()
        );
    }

    let api_key = response.json::<SandboxApiKey>()?.apiKey;

    Ok(SandboxUser {
        username: user_id_string,
        password: api_key,
        callback_host: callback_host.to_string(),
    })
}

pub fn main() -> Result<()> {
    let cli_config = CliConfig::from_args();

    let sandbox_credentials = create_sandbox_credentials(&cli_config)?;

    println!("{:?}", &sandbox_credentials);

    Ok(())
}
