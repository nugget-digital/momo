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
use util::rm_lead_char;

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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Country {
    code: String,
    prefix: String,
    non_prefix_digits: usize,
}

lazy_static! {
    static ref ONLY_NUMBERS: Regex = Regex::new("[^0-9]+").unwrap();
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
struct Msisdn(String);

impl Msisdn {
    fn new(
        mobile_number: &str,
        default_country: &Country,
        supported_countries: Vec<&Country>,
    ) -> Result<Msisdn> {
        let rebase = ONLY_NUMBERS.replace_all(mobile_number, "");

        let mut rebase: &str = rm_lead_char(&rebase, '0', true);

        // check if rebase startswith a known prefix and has the correct number of non_prefix_digits
        // if yes return rebase
        // else procedd with default_country
        for supported_country in supported_countries {
            if rebase.starts_with(&supported_country.prefix)
                && rebase.len() == supported_country.non_prefix_digits + 3usize
            {
                return Ok(Msisdn(rebase.to_string()));
            }
        }

        if rebase.len() < default_country.non_prefix_digits {
            bail!(
                "mobile number {} has too few \
                 non prefix digits for default {:?}",
                mobile_number,
                default_country
            );
        } else if rebase.len() > default_country.non_prefix_digits {
            for c in default_country.prefix.chars() {
                rebase = rm_lead_char(rebase, c, false);
            }

            rebase = rm_lead_char(rebase, '0', false);

            if rebase.len() != default_country.non_prefix_digits {
                bail!(
                    "mobile number {} has an incorrect number of \
                     non prefix digits for default {:?}",
                    mobile_number,
                    default_country
                );
            }
        }

        Ok(Msisdn(format!("{}{}", default_country.prefix, rebase)))
    }
}

impl fmt::Display for Msisdn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (self.0).fmt(f)
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    #[serde(rename(deserialize = "available_balance"))]
    availableBalance: String,
    currency: String,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
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

    lazy_static! {
        static ref GHANA: Country = Country {
            code: String::from("GH"),
            prefix: String::from("233"),
            non_prefix_digits: 9usize,
        };
        static ref NIGERIA: Country = Country {
            code: String::from("NG"),
            prefix: String::from("234"),
            non_prefix_digits: 8usize,
        };
        static ref GHANA_MSISDN: Regex =
            Regex::new("^233[1-9]{2}[0-9]{7}$").expect("GHANA_MSISDN");
        static ref NIGERIA_MSISDN: Regex =
            Regex::new("^234[1-9]{2}[0-9]{6}$").expect("NIGERIA_MSISDN");
    }

    proptest! {
        #[test]
        fn gh_mobile_number_to_msisdn(s in "(:?(:?(:?\\+|00)233-?)|0)?[1-9]{2}-?[0-9]{3}-?[0-9]{4}") {
            let msisdn = Msisdn::new(&s, &GHANA, vec![&GHANA]).expect("Msisdn::new");

            assert!(GHANA_MSISDN.is_match(&msisdn.to_string()));
        }

        #[test]
        fn ng_mobile_number_to_msisdn(s in "(:?(:?(:?\\+|00)234-?)|0)?[1-9]{2}-?[0-9]{3}-?[0-9]{3}") {
            let msisdn = Msisdn::new(&s, &NIGERIA, vec![&NIGERIA]).expect("Msisdn::new");

            assert!(NIGERIA_MSISDN.is_match(&msisdn.to_string()));
        }

        #[test]
        fn msisdn_normalization_using_default_country(s in "0?[1-9]{2}-?[0-9]{3}-?[0-9]{4}") {
            let msisdn = Msisdn::new(&s, &GHANA, vec![&GHANA]).expect("Msisdn::new");

            assert!(GHANA_MSISDN.is_match(&msisdn.to_string()));
        }

        #[test]
        fn msisdn_normalization_using_non_default_country(s in "(:?(:?\\+|00)234-?)[1-9]{2}-?[0-9]{3}-?[0-9]{3}") {
            let msisdn = Msisdn::new(&s, &GHANA, vec![&GHANA, &NIGERIA]).expect("Msisdn::new");

            assert!(NIGERIA_MSISDN.is_match(&msisdn.to_string()));
        }

        #[test]
        fn msisdn_normalization_fails_on_short_numbers(s in "[0-9]{1,7}") {
            assert!(Msisdn::new(&s, &GHANA, vec![&GHANA, &NIGERIA]).is_err());
        }

        #[test]
        fn msisdn_normalization_fails_on_large_numbers(s in "[0-9]{13,}") {
            assert!(Msisdn::new(&s, &GHANA, vec![&GHANA, &NIGERIA]).is_err());
        }

        #[test]
        fn msisdn_normalization_fails_on_unicode(s in "\\PC*") {
            assert!(Msisdn::new(&s, &GHANA, vec![&GHANA, &NIGERIA]).is_err());
        }

        #[test]
        fn payment_status_from_str_fails_on_unicode(s in "\\PC*")  {
            assert!(PaymentStatus::from_str(&s).is_err());
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
