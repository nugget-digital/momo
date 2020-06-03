use std::fmt;
use std::str::FromStr;

use anyhow::{bail, Error, Result};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use url::Url;

mod util;
use util::strip_lead_char;

lazy_static! {
    pub static ref FALLBACK_CALLBACK_URL: Url =
        Url::parse("https://www.mocky.io/v2/5ec0fa1c2f000079004c86fb")
            .expect("url");
    static ref NUMBERS_ONLY: Regex = Regex::new("[^0-9]+").expect("regex");
}

pub const FALLBACK_CALLBACK_HOST: &str = "www.mocky.io";
// pub const FALLBACK_CALLBACK_URL: &str =
//     "https://www.mocky.io/v2/5ec0fa1c2f000079004c86fb";
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
        let payment_status: PaymentStatus = match status {
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
pub enum Currency {
    Cedi,
    Naira,
}

impl FromStr for Currency {
    type Err = Error;

    fn from_str(currency_str: &str) -> Result<Currency> {
        let currency: Currency = match currency_str {
            "GHS" => Currency::Cedi,
            "NGN" => Currency::Naira,
            _ => bail!("unknown currency {:?}", currency_str),
        };

        Ok(currency)
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s: &str = match self {
            Currency::Cedi => "GHS",
            Currency::Naira => "NGN",
        };

        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Country {
    pub code: String,
    pub prefix: String,
    pub non_prefix_digits: usize,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Msisdn(String);

impl Msisdn {
    pub fn parse(
        mobile_number: &str,
        default_country: &Country,
        alternate_countries: Option<Vec<&Country>>,
    ) -> Result<Msisdn> {
        let rebase: &str = &NUMBERS_ONLY.replace_all(mobile_number, "");

        let mut rebase: &str = strip_lead_char(&rebase, '0', true);

        if let Some(countries) = alternate_countries {
            for country in countries {
                if rebase.starts_with(&country.prefix)
                    && rebase.len() == 3usize + country.non_prefix_digits
                {
                    return Ok(Msisdn(rebase.to_string()));
                }
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
            for character in default_country.prefix.chars() {
                rebase = strip_lead_char(rebase, character, false);
            }

            rebase = strip_lead_char(rebase, '0', false);

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
    #[serde(rename = "availableBalance")]
    amount: String,
    // TODO: make this a public enum
    currency: String,
}

#[cfg(test)]
mod mtn_momo_common_unit_tests {
    mod msisdn {
        // use std::str::FromStr;

        use crate::{Country, Msisdn};
        use lazy_static::lazy_static;
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
            fn from_gh_mobile_number(s in "(:?(:?(:?\\+|00)233-?)|0)?[1-9]{2}-?[0-9]{3}-?[0-9]{4}") {
                let msisdn: Msisdn =
                    Msisdn::parse(&s, &GHANA, None).expect("msisdn");

                assert!(GHANA_MSISDN.is_match(&msisdn.to_string()));
            }

            #[test]
            fn from_ng_mobile_number(s in "(:?(:?(:?\\+|00)234-?)|0)?[1-9]{2}-?[0-9]{3}-?[0-9]{3}") {
                let msisdn: Msisdn =
                    Msisdn::parse(&s, &NIGERIA, None).expect("msisdn");

                assert!(NIGERIA_MSISDN.is_match(&msisdn.to_string()));
            }

            #[test]
            fn normalization_using_default_country(s in "0?[1-9]{2}-?[0-9]{3}-?[0-9]{4}") {
                let msisdn: Msisdn =
                    Msisdn::parse(&s, &GHANA, None).expect("msisdn");

                assert!(GHANA_MSISDN.is_match(&msisdn.to_string()));
            }

            #[test]
            fn normalization_using_alternate_country(s in "(:?(:?\\+|00)234-?)[1-9]{2}-?[0-9]{3}-?[0-9]{3}") {
                let msisdn: Msisdn =
                    Msisdn::parse(&s, &GHANA, Some(vec![&NIGERIA])).expect("msisdn");

                assert!(NIGERIA_MSISDN.is_match(&msisdn.to_string()));
            }

            #[test]
            fn normalization_fails_on_too_short_gh_numbers(s in "[0-9]{1,7}") {
                assert!(Msisdn::parse(&s, &GHANA, None).is_err());
            }

            #[test]
            fn normalization_fails_on_too_short_ng_numbers(s in "[0-9]{1,6}") {
                assert!(Msisdn::parse(&s, &NIGERIA, None).is_err());
            }

            #[test]
            fn normalization_fails_on_too_large_gh_numbers(s in "0*233[1-9][0-9]{9,}") {
                assert!(Msisdn::parse(&s, &GHANA, None).is_err());
            }

            #[test]
            fn normalization_fails_on_too_large_ng_numbers(s in "0*234[1-9][0-9]{8,}") {
                assert!(Msisdn::parse(&s, &NIGERIA, None).is_err());
            }

            #[test]
            fn normalization_fails_on_unicode(s in "\\PC*") {
                assert!(Msisdn::parse(&s, &GHANA, None).is_err());
            }
        }
    }

    mod payment_status {
        use std::str::FromStr;

        use crate::PaymentStatus;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn from_str_fails_on_unicode(s in "\\PC*")  {
                assert!(PaymentStatus::from_str(&s).is_err());
            }
        }
    }
}
