use common::*;
use lazy_static::lazy_static;
use mini::*;
use std::sync::{Mutex, MutexGuard};

lazy_static! {
    static ref CONFIG: Config = Config {
        username: env!("MTN_MOMO_USERNAME").to_string(),
        password: env!("MTN_MOMO_PASSWORD").to_string(),
        subscription_key: env!("MTN_MOMO_SUBSCRIPTION_KEY").to_string(),
        // for production pass https://momodeveloper.mtn.com/ as base_url
        // falls back to https://sandbox.momodeveloper.mtn.com/
        base_url: None,
        // for production pass your api hostname (nugget.digital)
        // falls back to www.mocky.io
        callback_host: None,
    };
    static ref CLIENT: Mutex<Client> =
        Mutex::new(Client::new(&CONFIG).expect("client"));
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
