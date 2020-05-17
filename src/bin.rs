use anyhow::{anyhow, Result};
use http::StatusCode;
use reqwest::blocking;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};
use structopt::StructOpt;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct SandboxCredentials {
    username: String,
    password: String,
}

#[derive(Debug, StructOpt)]
struct Konfu {
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

fn create_sandbox_credentials(
    callback_host: &str,
    subscription_key: &str,
) -> Result<SandboxCredentials> {
    let client = blocking::Client::new();

    let body = json!({ "providerCallbackHost": callback_host }).to_string();

    let user_id = Uuid::new_v4();
    let user_id_string = user_id.to_string();

    let response = client
        .post("https://sandbox.momodeveloper.mtn.com/v1_0/apiuser")
        .header("X-Reference-Id", &user_id_string)
        .header("Ocp-Apim-Subscription-Key", subscription_key)
        .json(&body)
        .send()?;

    if response.status() != StatusCode::CREATED {
        return Err(anyhow!(
            "creating a sandbox user failed - http status {:?}",
            response.status()
        ));
    }

    let url = format!(
        "https://sandbox.momodeveloper.mtn.com/v1_0/apiuser/{}/apikey",
        &user_id_string,
    );

    let response = client
        .post(&url)
        .header("X-Reference-Id", &user_id_string)
        .header("Ocp-Apim-Subscription-Key", subscription_key)
        .send()?;

    if response.status() != StatusCode::CREATED {
        return Err(anyhow!(
            "creating a sandbox api key failed - http status {:?}",
            response.status()
        ));
    }

    let api_key = response.json::<SandboxApiKey>()?.apiKey;

    Ok(SandboxCredentials {
        username: user_id_string,
        password: api_key,
    })
}

pub fn main() -> Result<()> {
    let konfu = Konfu::from_args();

    let callback_host = if let Some(host) = &konfu.callback_host {
        host
    } else {
        // callback_host must be the host for all callback urls used
        // mock PUT https://www.mocky.io/v2/5ec0fa1c2f000079004c86fb
        // NOTE: fallback hardcoded bc required when creating a sandbox user
        "mocky.io"
    };

    let sandbox_credentials =
        create_sandbox_credentials(callback_host, &konfu.subscription_key)?;

    println!("{}", to_string(&sandbox_credentials)?);

    Ok(())
}
