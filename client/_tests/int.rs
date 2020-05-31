use anyhow::Result;
use mini_mtn_momo::*;

#[test]
fn constructing_a_client() -> Result<()> {
    let config: Config = Config {
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

    let client: Client = Client::new(&config)?;

    assert_eq!(&client.target_environment, common::SANDBOX);
    assert_eq!(&client.base_url, common::SANDBOX_BASE_URL);
    assert_eq!(&client.callback_host, common::FALLBACK_CALLBACK_HOST);

    Ok(())
}
