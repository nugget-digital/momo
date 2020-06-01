use std::str::FromStr;

use anyhow::{bail, Result};
use common::*;
use http::StatusCode;
use log::{debug, info, trace, warn};
use reqwest::blocking;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub username: String,
    pub password: String,
    pub subscription_key: String,
    pub base_url: Option<String>,
    pub callback_host: Option<String>,
}

#[derive(Debug)]
pub struct Client {
    http_client: blocking::Client,
    pub target_environment: String,
    username: String,
    password: String,
    subscription_key: String,
    collections_access_token: String,
    pub base_url: String,
    pub callback_host: String,
    reauthorize: bool,
}

#[derive(Deserialize)]
struct Authorization {
    access_token: String,
    token_type: String,
    expires_in: u64,
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
        msisdn: &str,
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
        let http_client: blocking::Client = blocking::Client::builder()
            .http1_title_case_headers()
            .build()?;

        let base_url: String;
        let target_environment: &str;

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
            debug!(
                "[mini-mtn-momo] using fallback sandbox environment \
                located @ {}",
                SANDBOX_BASE_URL
            );

            base_url = SANDBOX_BASE_URL.to_string();
            target_environment = SANDBOX;
        };

        let callback_host: &str = if let Some(domain) = &config.callback_host {
            domain
        } else {
            debug!(
                "[mini-mtn-momo] using fallback callback host \"{}\"",
                FALLBACK_CALLBACK_HOST
            );

            FALLBACK_CALLBACK_HOST
        };

        let mut client: Client = Client {
            http_client,
            target_environment: target_environment.to_string(),
            username: config.username.clone(),
            password: config.password.clone(),
            subscription_key: config.subscription_key.clone(),
            collections_access_token: "".to_string(),
            base_url,
            callback_host: callback_host.to_string(),
            reauthorize: true,
        };

        client.authorize_collections()?;

        Ok(client)
    }

    fn authorize_collections(&mut self) -> Result<&Client> {
        self.reauthorize = false;

        let url: String = format!("{}collection/token/", &self.base_url);

        let response: blocking::Response = self
            .http_client
            .post(&url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .header("Content-Length", "0")
            .send()?;

        if response.status() != StatusCode::OK {
            bail!(
                "authorizing collections failed - http status {:?}\n{}",
                response.status(),
                response.text()?,
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
        msisdn: &str,
        callback_url: Option<&str>,
    ) -> Result<Uuid> {
        let url: String =
            format!("{}collection/v1_0/requesttopay/", &self.base_url);

        let reference_id: Uuid = Uuid::new_v4();
        let reference_id_string: String = reference_id.to_string();

        let cb_url: &str = if let Some(url) = callback_url {
            url
        } else if self.callback_host.ends_with("mocky.io") {
            debug!(
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

        let body: String = json!({
            "amount": amount,
            "currency": currency,
            "externalId": &reference_id_string,
            "payer": {
              "partyIdType": "MSISDN",
              "partyId": msisdn,
            },
            "payerMessage": "it's time to pay :)",
            "payeeNote": "TODO",
        })
        .to_string();

        let response: blocking::Response = self
            .http_client
            .post(&url)
            .bearer_auth(&self.collections_access_token)
            .header("X-Callback-Url", cb_url)
            .header("X-Reference-Id", &reference_id_string)
            .header("X-Target-Environment", &self.target_environment)
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .header("Content-Type", "application/json")
            .header("Content-Length", body.chars().count())
            .body(body)
            .send()?;

        let status: StatusCode = response.status();

        if status == StatusCode::ACCEPTED {
            Ok(reference_id)
        } else if status == StatusCode::UNAUTHORIZED && self.reauthorize {
            debug!("currently unauthorized, attempting reauthorization...");

            self.authorize_collections()?;

            self.request_to_pay(amount, currency, msisdn, callback_url)
        } else {
            bail!(
                "payment request failed - http status {:?} - \
                reference id {}\n{}",
                response.status(),
                reference_id_string,
                response.text()?
            );
        }
    }

    fn request_to_pay_status(
        &mut self,
        reference_id: &Uuid,
    ) -> Result<PaymentStatus> {
        let url: String = format!(
            "{}collection/v1_0/requesttopay/{}",
            &self.base_url,
            reference_id.to_string()
        );

        let response: blocking::Response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.collections_access_token)
            .header("X-Target-Environment", &self.target_environment)
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .send()?;

        let status: StatusCode = response.status();

        if status == StatusCode::OK {
            let payment_status_string: String =
                response.json::<Payment>()?.status;

            let payment_status: PaymentStatus =
                PaymentStatus::from_str(&payment_status_string[..])?;

            Ok(payment_status)
        } else if status == StatusCode::UNAUTHORIZED && self.reauthorize {
            debug!("currently unauthorized, attempting reauthorization...");

            self.authorize_collections()?;

            self.request_to_pay_status(&reference_id)
        } else {
            bail!(
                "requesting payment status failed - http status {:?} - \
                    reference id {}\n{}",
                response.status(),
                reference_id.to_string(),
                response.text()?
            );
        }
    }

    fn get_balance(&mut self) -> Result<Balance> {
        let url: String =
            format!("{}collection/v1_0/account/balance", &self.base_url);

        let response: blocking::Response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.collections_access_token)
            .header("X-Target-Environment", &self.target_environment)
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .send()?;

        let status: StatusCode = response.status();

        if status == StatusCode::OK {
            let balance = response.json::<Balance>()?;

            Ok(balance)
        } else if status == StatusCode::UNAUTHORIZED && self.reauthorize {
            debug!("currently unauthorized, attempting reauthorization...");

            self.authorize_collections()?;

            self.get_balance()
        } else {
            bail!(
                "getting wallet balance failed - http status {:?}\n{}",
                response.status(),
                response.text()?
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtripping_payment_status_pending() -> () {
        let string: String = PaymentStatus::Pending.to_string();

        assert_eq!(string, "PENDING");

        let status: PaymentStatus =
            PaymentStatus::from_str(&string).expect("PaymentStatus::Pending");

        assert_eq!(status, PaymentStatus::Pending);
    }

    #[test]
    fn roundtripping_payment_status_rejected() -> () {
        let string: String = PaymentStatus::Rejected.to_string();

        assert_eq!(string, "FAILED");

        let status: PaymentStatus =
            PaymentStatus::from_str(&string).expect("PaymentStatus::Rejected");

        assert_eq!(status, PaymentStatus::Rejected);
    }

    #[test]
    fn roundtripping_payment_status_resolved() -> () {
        let string: String = PaymentStatus::Resolved.to_string();

        assert_eq!(string, "SUCCESSFUL");

        let status: PaymentStatus =
            PaymentStatus::from_str(&string).expect("PaymentStatus::Resolved");

        assert_eq!(status, PaymentStatus::Resolved);
    }

    #[test]
    fn payment_status_from_str_fails_on_unknown_status() {
        assert!(PaymentStatus::from_str("UNKNOWN").is_err());
    }
}
