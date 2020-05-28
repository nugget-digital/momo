use anyhow::{bail, Error, Result};
use http::StatusCode;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::blocking;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use std::fmt;
use std::str::FromStr;

pub mod common;
mod util;

use common::*;
use util::rm_lead_char_plus;

#[repr(C)]
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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

impl fmt::Display for PaymentStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s: &str = match self {
            PaymentStatus::Resolved => "SUCCESSFUL",
            PaymentStatus::Rejected => "FAILED",
            PaymentStatus::Pending => "PENDING",
        };

        write!(f, "{}", s)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum CountryCode {
    Ghana,
    Nigeria,
}

impl FromStr for CountryCode {
    type Err = Error;

    fn from_str(code: &str) -> Result<CountryCode> {
        let country_code = match code {
            "233" => CountryCode::Ghana,
            "419" => CountryCode::Nigeria,
            _ => bail!("unknown country code {:?}", code),
        };

        Ok(country_code)
    }
}

impl fmt::Display for CountryCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s: &str = match self {
            CountryCode::Ghana => "233",
            CountryCode::Nigeria => "419",
        };

        write!(f, "{}", s)
    }
}

// const ONLY_NUMBERS: Regex = Regex::new("[^0-9]+").unwrap();

lazy_static! {
    static ref ONLY_NUMBERS: Regex = Regex::new("[^0-9]+").unwrap();
}

// TODO: how to display msisdn to_string()
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
struct Msisdn(String);

impl Msisdn {
    fn new(country_code: CountryCode, mobile_number: &str) -> Result<Msisdn> {
        let numbers = ONLY_NUMBERS.replace(mobile_number, "");

        let rebase: &str = rm_lead_char_plus(&numbers, '0');

        // for each char in country_code do rm_lead_char_plus
        // country_code.to_string().chars().iter()
        for c in country_code.to_string().chars() {
            let rebase: &str = rm_lead_char_plus(rebase, c);
            let rebase: &str = rm_lead_char_plus(rebase, c);
        }
        // let rebase: &str = rm_lead_char_plus(rebase, '2');
        // let rebase: &str = rm_lead_char_plus(rebase, '3');

        let rebase: &str = rm_lead_char_plus(rebase, '0');

        let msisdn: String = format!("{}{}", country_code, rebase);

        Ok(Msisdn(msisdn))
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
    pub target_environment: String,
    username: String,
    password: String,
    subscription_key: String,
    collections_access_token: String,
    pub base_url: String,
    pub callback_host: String,
    reauthorize: bool,
    pub metadata: String,
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
            println!("[mini-mtn-momo] using fallback device id \"None\"");

            "None"
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use regex::Regex;

    proptest! {
        #[test]
        fn gh_mobile_number_to_msisdn(s in "\\+233-[1-9]{2}-[0-9]{3}-[0-9]{4}") {
            let expected: Regex = Regex::new("^233[^1-9]{2}[0-9]{7}$").expect("Regex::new");

            let msisdn = Msisdn::new(CountryCode::Ghana, &s).expect("Msisdn::new");

            assert!(expected.is_match(&format!("{}", msisdn)));
        }

        #[test]
        fn payment_status_from_str_fails_on_unicode(s in "\\PC*") {
            assert!(PaymentStatus::from_str(&s).is_err())
        }
    }

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
