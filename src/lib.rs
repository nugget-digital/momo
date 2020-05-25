use anyhow::{bail, Error, Result};
use http::StatusCode;
use reqwest::blocking;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use std::str::FromStr;

mod common;

use common::*;

#[repr(C)]
#[derive(Debug, Serialize, Deserialize)]
pub enum PaymentStatus {
    Resolved,
    Rejected,
    Pending,
}

impl FromStr for PaymentStatus {
    type Err = Error;

    fn from_str(status: &str) -> Result<PaymentStatus> {
        let payment_status = match status {
            "SUCCESSFUL" => PaymentStatus::Resolved,
            "FAILED" => PaymentStatus::Rejected,
            "PENDING" => PaymentStatus::Pending,
            _ => bail!("unknown payment status {:?}", status),
        };

        Ok(payment_status)
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Balance {
    #[serde(rename(deserialize = "available_balance"))]
    availableBalance: String,
    currency: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub username: String,
    pub password: String,
    pub subscription_key: String,
    pub base_url: Option<String>,
    pub callback_host: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Debug)]
pub struct Client {
    http_client: blocking::Client,
    target_environment: String,
    username: String,
    password: String,
    subscription_key: String,
    collections_access_token: String,
    base_url: String,
    callback_host: String,
    reauthorize: bool,
    metadata: String,
}

#[derive(Deserialize)]
struct Authorization {
    access_token: String,
    token_type: String,
    expires_in: String,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct Payer {
    partyIdType: String,
    partyId: u64,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct Payment {
    amount: u64,
    currency: String,
    financialTransactionId: u64,
    externalId: u64,
    payer: Payer,
    status: String,
}

pub trait IClient {
    fn new(config: &Config) -> Result<Client>;
    fn authorize_collections(&mut self) -> Result<&Client>;
    fn request_to_pay(
        &mut self,
        amount: u64,
        currency: &str,
        mobile_number: &str,
        callback_url: Option<&str>,
    ) -> Result<Uuid>;
    fn request_to_pay_status(
        &mut self,
        reference_id: &Uuid,
    ) -> Result<PaymentStatus>;
    fn get_balance(&mut self) -> Result<Balance>;
}

impl IClient for Client {
    fn new(config: &Config) -> Result<Client> {
        let http_client = blocking::Client::builder()
            .http1_title_case_headers()
            .build()?;

        let base_url;
        let target_environment;

        if let Some(url) = &config.base_url {
            if url.ends_with("/") {
                base_url = url.clone();
            } else {
                base_url = format!("{}/", url);
            };

            if url.starts_with(PRODUCTION_BASE_URL) {
                target_environment = PRODUCTION;
            } else {
                target_environment = SANDBOX;
            };
        } else {
            println!(
                "[mini-mtn-momo] using fallback sandbox environment \
                located @ {}",
                SANDBOX_BASE_URL
            );

            base_url = SANDBOX_BASE_URL.to_string();
            target_environment = SANDBOX;
        };

        let callback_host = if let Some(domain) = &config.callback_host {
            domain
        } else {
            println!(
                "[mini-mtn-momo] using fallback callback host \"{}\"",
                FALLBACK_CALLBACK_HOST
            );

            FALLBACK_CALLBACK_HOST
        };

        let device_id = if let Some(id) = &config.device_id {
            id
        } else {
            println!("[mini-mtn-momo] using fallback device id \"unknown\"");

            "unknown"
        };

        let mut client = Client {
            http_client,
            target_environment: target_environment.to_string(),
            username: config.username.clone(),
            password: config.password.clone(),
            subscription_key: config.subscription_key.clone(),
            collections_access_token: "".to_string(),
            base_url,
            callback_host: callback_host.to_string(),
            reauthorize: true,
            metadata: json!({ "device_id": device_id }).to_string(),
        };

        client.authorize_collections()?;

        Ok(client)
    }

    fn authorize_collections(&mut self) -> Result<&Client> {
        self.reauthorize = false;

        let url = format!("{}collection/token/", &self.base_url);

        let response = self
            .http_client
            .post(&url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .send()?;

        if response.status() != StatusCode::OK {
            bail!(
                "authorizing collections failed - http status {:?}",
                response.status()
            );
        } else {
            self.collections_access_token =
                response.json::<Authorization>()?.access_token;

            self.reauthorize = true;

            Ok(self)
        }
    }

    fn request_to_pay(
        &mut self,
        amount: u64,
        currency: &str,
        mobile_number: &str,
        callback_url: Option<&str>,
    ) -> Result<Uuid> {
        let url = format!("{}collection/v1_0/requesttopay/", &self.base_url);

        let reference_id = Uuid::new_v4();
        let reference_id_string = reference_id.to_string();

        let cb_url = if let Some(url) = callback_url {
            url
        } else if self.callback_host.ends_with("mocky.io") {
            println!(
                "[mini-mtn-momo] using fallback callback url \"{}\"",
                FALLBACK_CALLBACK_URL
            );

            FALLBACK_CALLBACK_URL
        } else {
            bail!(
                "when having specified a custom callback host a callback url \
                 with the same host is required for every request to pay"
            );
        };

        let response = self
            .http_client
            .post(&url)
            .bearer_auth(&self.collections_access_token)
            .header("X-Callback-Url", cb_url)
            .header("X-Reference-Id", &reference_id_string)
            .header("X-Target-Environment", &self.target_environment)
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .json(&json!({
                "amount": amount,
                "currency": currency,
                "externalId": &reference_id_string,
                "payer": {
                  "partyIdType": "MSISDN",
                  // TODO: normalize mobile number
                  "partyId": mobile_number,
                },
                "payerMessage": "it's time to pay :)",
                "payeeNote": &self.metadata,
            }))
            .send()?;

        let status = response.status();

        if status == StatusCode::ACCEPTED {
            Ok(reference_id)
        } else if status == StatusCode::UNAUTHORIZED && self.reauthorize {
            println!("currently unauthorized, attempting reauthorization...");

            self.authorize_collections()?;

            self.request_to_pay(amount, currency, mobile_number, callback_url)
        } else {
            bail!(
                "payment request failed - http status {:?} - reference id {}",
                response.status(),
                reference_id_string,
            );
        }
    }

    fn request_to_pay_status(
        &mut self,
        reference_id: &Uuid,
    ) -> Result<PaymentStatus> {
        let url = format!(
            "{}collection/v1_0/requesttopay/{}",
            &self.base_url,
            reference_id.to_string()
        );

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.collections_access_token)
            .header("X-Target-Environment", &self.target_environment)
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .send()?;

        let status = response.status();

        if status == StatusCode::OK {
            let payment_status_string = response.json::<Payment>()?.status;

            let payment_status =
                PaymentStatus::from_str(&payment_status_string[..])?;

            Ok(payment_status)
        } else if status == StatusCode::UNAUTHORIZED && self.reauthorize {
            println!("currently unauthorized, attempting reauthorization...");

            self.authorize_collections()?;

            self.request_to_pay_status(&reference_id)
        } else {
            bail!(
                    "requesting payment status failed - http status {:?} - reference id {}",
                    response.status(),
                    reference_id.to_string(),
                );
        }
    }

    fn get_balance(&mut self) -> Result<Balance> {
        let url = format!("{}collection/v1_0/account/balance", &self.base_url);

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.collections_access_token)
            .header("X-Target-Environment", &self.target_environment)
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .send()?;

        let status = response.status();

        if status == StatusCode::OK {
            let balance = response.json::<Balance>()?;

            Ok(balance)
        } else if status == StatusCode::UNAUTHORIZED && self.reauthorize {
            println!("currently unauthorized, attempting reauthorization...");

            self.authorize_collections()?;

            self.get_balance()
        } else {
            bail!(
                "getting wallet balance failed - http status {:?}",
                response.status(),
            );
        }
    }
}
