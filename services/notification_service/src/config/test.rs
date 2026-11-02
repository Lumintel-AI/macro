use super::*;
use serde_json::json;

fn config() -> serde_json::Value {
    json!({
        "DATABASE_URL": "postgres://localhost/macro",
        "INTERNAL_API_KEY": "test-key",
        "URL_SIGNING_HMAC": "test-hmac",
        "REDIS_URI": "redis://localhost",
        "LAST_ONLINE_REDIS_URI": "redis://localhost",
        "KAFKA_BROKERS": "localhost:9092",
        "ENVIRONMENT": "prod"
    })
}

#[test]
fn production_config_without_mobile_settings_loads() {
    let config: Config = serde_json::from_value(config()).unwrap();
    assert!(matches!(config.environment, Environment::Production));
    assert!(config.mobile_push_config().unwrap().is_none());
}

#[test]
fn blank_mobile_settings_disable_push() {
    let mut values = config();
    for key in [
        "APPLE_BUNDLE_ID",
        "SNS_APNS_PLATFORM_ARN",
        "SNS_FCM_PLATFORM_ARN",
        "SNS_APNS_VOIP_PLATFORM_ARN",
    ] {
        values[key] = json!("");
    }
    let config: Config = serde_json::from_value(values).unwrap();
    assert!(config.mobile_push_config().unwrap().is_none());
}

#[test]
fn partial_mobile_settings_fail_before_starting_clients() {
    let mut values = config();
    values["APPLE_BUNDLE_ID"] = json!("com.example.test");
    let config: Config = serde_json::from_value(values).unwrap();
    let error = config.mobile_push_config().err().unwrap().to_string();
    assert!(error.contains("SNS_APNS_PLATFORM_ARN"));
    assert!(error.contains("SNS_FCM_PLATFORM_ARN"));
    assert!(error.contains("SNS_APNS_VOIP_PLATFORM_ARN"));
}

#[test]
fn configured_mobile_settings_keep_platform_values() {
    let mut values = config();
    values["APPLE_BUNDLE_ID"] = json!("com.example.test");
    values["SNS_APNS_PLATFORM_ARN"] = json!("arn:aws:sns:us-east-2:123456789012:app/APNS/test");
    values["SNS_FCM_PLATFORM_ARN"] = json!("arn:aws:sns:us-east-2:123456789012:app/GCM/test");
    values["SNS_APNS_VOIP_PLATFORM_ARN"] =
        json!("arn:aws:sns:us-east-2:123456789012:app/APNS_VOIP/test");
    let config: Config = serde_json::from_value(values.clone()).unwrap();
    let mobile = config.mobile_push_config().unwrap().unwrap();
    assert_eq!(mobile.bundle_id, values["APPLE_BUNDLE_ID"]);
    assert_eq!(
        mobile.platforms.apns_platform_arn,
        values["SNS_APNS_PLATFORM_ARN"]
    );
    assert_eq!(
        mobile.platforms.fcm_platform_arn,
        values["SNS_FCM_PLATFORM_ARN"]
    );
    assert_eq!(
        mobile.platforms.apns_voip_platform_arn,
        values["SNS_APNS_VOIP_PLATFORM_ARN"]
    );
}
