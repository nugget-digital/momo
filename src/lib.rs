use anyhow::{anyhow, Result};
use http::StatusCode;
use reqwest::blocking;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[repr(C)]
#[derive(Debug, Serialize, Deserialize)]
pub enum PaymentStatus {
    Resolved,
    Rejected,
    Pending,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Balance {
    availableBalance: String,
    currency: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub username: String,
    pub password: String,
    pub subscription_key: String,
    pub base_url: Option<String>,
    pub callback_url: Option<String>,
}

#[derive(Debug)]
pub struct Client {
    config: Config,
    http_client: blocking::Client,
    collections_access_token: String,
    target_environment: String,
    base_url: String,
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
    fn request_to_pay(
        &self,
        amount: u64,
        mobile_number: String,
    ) -> Result<Uuid>;
    fn request_to_pay_status(
        &self,
        reference_id: Uuid,
    ) -> Result<PaymentStatus>;
    fn get_balance(&self) -> Result<Balance>;
    fn authorize_collections(&mut self) -> Result<&Self>;
}

// TODO: retry once on 401 and 403
impl IClient for Client {
    fn new(config: &Config) -> Result<Client> {
        let http_client = blocking::Client::new();

        let target_environment;
        let base_url;

        if let Some(url) = &config.base_url {
            if url.ends_with("/") {
                base_url = url.clone();
            } else {
                base_url = format!("{}/", url);
            };

            if url.starts_with("https://momodeveloper.mtn.com") {
                target_environment = "production";
            } else {
                target_environment = "sandbox";
            }
        } else {
            base_url = "https://sandbox.momodeveloper.mtn.com/".to_string();
            target_environment = "sandbox";
        };

        Ok(Client {
            config: Config {
                username: config.username.clone(),
                password: config.password.clone(),
                subscription_key: config.subscription_key.clone(),
                base_url: config.base_url.clone(),
                callback_url: config.callback_url.clone(),
            },
            http_client,
            collections_access_token: "".to_string(),
            target_environment: target_environment.to_string(),
            base_url,
        })
    }

    fn authorize_collections(&mut self) -> Result<&Self> {
        let url = format!("{}collection/token/", &self.base_url);

        let response = self
            .http_client
            .post(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .header("Ocp-Apim-Subscription-Key", &self.config.subscription_key)
            .json(
                &json!({
                    "": ""
                })
                .to_string(),
            )
            .send()?;

        if response.status() != StatusCode::OK {
            return Err(anyhow!(
                "authorizing collections failed - http status {:?}",
                response.status()
            ));
        } else {
            self.collections_access_token =
                response.json::<Authorization>()?.access_token;

            Ok(self)
        }
    }

    fn request_to_pay(
        &self,
        _amount: u64,
        _mobile_number: String,
    ) -> Result<Uuid> {
        let url = format!("{}collection/v1_0/requesttopay/", &self.base_url);

        let reference_id = Uuid::new_v4();

        let response = self
            .http_client
            .post(&url)
            .bearer_auth(&self.collections_access_token)
            .header(
                "X-Callback-Url",
                self.config.callback_url.as_ref().unwrap_or(&"".to_string()),
            )
            .header("X-Reference-Id", reference_id.to_string())
            .header("X-Target-Environment", &self.target_environment)
            .header("Ocp-Apim-Subscription-Key", &self.config.subscription_key)
            .json("TODO")
            .send()?;

        if response.status() != StatusCode::ACCEPTED {
            Err(anyhow!(
                "payment request failed - http status {:?} - reference id {}",
                response.status(),
                reference_id.to_string(),
            ))
        } else {
            Ok(reference_id)
        }
    }

    fn request_to_pay_status(
        &self,
        reference_id: Uuid,
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
            .header("Ocp-Apim-Subscription-Key", &self.config.subscription_key)
            .send()?;

        if response.status() != StatusCode::OK {
            Err(anyhow!(
                "requesting payment status failed - http status {:?} - reference id {}",
                response.status(),
                reference_id.to_string(),
            ))
        } else {
            let status = response.json::<Payment>()?.status;
            let payment_status = match &status[..] {
                "SUCCESSFUL" => PaymentStatus::Resolved,
                "FAILED" => PaymentStatus::Rejected,
                "PENDING" => PaymentStatus::Pending,
                _ => {
                    return Err(anyhow!("unknown payment status {:?}", status))
                }
            };

            Ok(payment_status)
        }
    }

    fn get_balance(&self) -> Result<Balance> {
        let url = format!("{}collection/v1_0/account/balance", &self.base_url);

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.collections_access_token)
            .header("X-Target-Environment", &self.target_environment)
            .header("Ocp-Apim-Subscription-Key", &self.config.subscription_key)
            .send()?;

        if response.status() != StatusCode::OK {
            Err(anyhow!(
                "getting wallet balance failed - http status {:?}",
                response.status()
            ))
        } else {
            let balance = response.json::<Balance>()?;

            Ok(balance)
        }
    }
}
