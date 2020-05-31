use std::fmt;
use std::str::FromStr;

use anyhow::{bail, Error, Result};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

mod util;
use util::rm_lead_char;

pub const FALLBACK_CALLBACK_HOST: &str = "www.mocky.io";
pub const FALLBACK_CALLBACK_URL: &str =
    "https://www.mocky.io/v2/5ec0fa1c2f000079004c86fb";
// NOTE: the production base url does not have a trailing slash here as
// we use it for an equality check only, not for constructing an url
pub const PRODUCTION_BASE_URL: &str = "https://momodeveloper.mtn.com";
pub const SANDBOX_BASE_URL: &str = "https://sandbox.momodeveloper.mtn.com/";
pub const PRODUCTION: &str = "production";
pub const SANDBOX: &str = "sandbox";

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
pub struct Msisdn(String);

impl Msisdn {
    fn new(
        mobile_number: &str,
        default_country: &Country,
        supported_countries: Vec<&Country>,
    ) -> Result<Msisdn> {
        let rebase = ONLY_NUMBERS.replace_all(mobile_number, "");

        let mut rebase: &str = rm_lead_char(&rebase, '0', true);

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
    // TODO: make this an int of cents
    #[serde(rename = "amount")]
    availableBalance: String,
    // TODO: make this a public enum
    currency: String,
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
}
