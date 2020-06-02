use std::sync::{Mutex, MutexGuard};

use common::*;
use lazy_static::lazy_static;
use mini::*;
use uuid::Uuid;

lazy_static! {
    static ref CONFIG: Config = Config {
        username: env!("MTN_MOMO_SANDBOX_USERNAME").to_string(),
        password: env!("MTN_MOMO_SANDBOX_PASSWORD").to_string(),
        subscription_key: env!("MTN_MOMO_SANDBOX_SUBSCRIPTION_KEY").to_string(),
        // for production pass https://momodeveloper.mtn.com/ as base_url
        // falls back to https://sandbox.momodeveloper.mtn.com/
        base_url: None,
        // for production pass your api hostname (nugget.digital)
        // falls back to www.mocky.io
        callback_host: None,
    };
    static ref CLIENT: Mutex<Client> =
        Mutex::new(Client::new(&CONFIG).expect("client"));
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
}

#[test]
fn part1_creating_a_client() {
    // NOTE: lazy_static! silently initalizes the client on first access
    let client: MutexGuard<Client> = CLIENT.lock().expect("mutex client");

    assert_eq!(&client.target_environment, common::SANDBOX);
    assert_eq!(&client.base_url, common::SANDBOX_BASE_URL);
    assert_eq!(&client.callback_host, common::FALLBACK_CALLBACK_HOST);
}

#[test]
fn part2_getting_balance() {
    let mut client: MutexGuard<Client> = CLIENT.lock().expect("mutex client");

    let _balance: Balance = client.get_balance().expect("balance");
}

#[test]
fn part3_request_to_pay_without_a_callback() {
    let mut client: MutexGuard<Client> = CLIENT.lock().expect("mutex client");

    let msisdn: Msisdn =
        Msisdn::parse("0542373722", &GHANA, None).expect("msisdn");

    let _uuid: Uuid = client
        .request_to_pay(419u64, Currency::Cedi, &msisdn, None)
        .expect("request_to_pay");
}

#[test]
fn part4_request_to_pay_with_a_callback() {
    let mut client: MutexGuard<Client> = CLIENT.lock().expect("mutex client");

    let msisdn: Msisdn =
        Msisdn::parse("0542373722", &GHANA, None).expect("msisdn");

    let _uuid: Uuid = client
        .request_to_pay(
            419u64,
            Currency::Cedi,
            &msisdn,
            Some(&FALLBACK_CALLBACK_URL),
        )
        .expect("request_to_pay");
}
