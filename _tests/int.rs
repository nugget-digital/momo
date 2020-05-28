use anyhow::Result;
use mini_mtn_momo::*;

#[test]
fn constructing_a_client() -> Result<()> {
    let config: Config = Config {
        username: env!("MTN_MOMO_USERNAME").to_string(),
        password: env!("MTN_MOMO_PASSWORD").to_string(),
        subscription_key: env!("MTN_MOMO_SUBSCRIPTION_KEY").to_string(),
        base_url: None,
        callback_host: None,
        device_id: Some("device_id".to_string()),
    };

    let client: Client = Client::new(&config)?;

    assert_eq!(&client.target_environment, common::SANDBOX);
    assert_eq!(&client.base_url, common::SANDBOX_BASE_URL);
    assert_eq!(&client.callback_host, common::FALLBACK_CALLBACK_HOST);
    assert_eq!(&client.metadata, "{\"device_id\":\"device_id\"}");

    Ok(())
}
