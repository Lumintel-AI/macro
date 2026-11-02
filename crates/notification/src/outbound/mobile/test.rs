use super::*;
use aws_sdk_sns::types::error::{
    EndpointDisabledException, InvalidParameterException, NotFoundException,
    PlatformApplicationDisabledException,
};

fn invalid_parameter(message: &str) -> PublishError {
    PublishError::InvalidParameterException(
        InvalidParameterException::builder()
            .message(message)
            .build(),
    )
}

#[test]
fn disabled_endpoint_is_unavailable() {
    let err = PublishError::EndpointDisabledException(
        EndpointDisabledException::builder()
            .message("Endpoint is disabled")
            .build(),
    );
    assert!(is_unavailable_endpoint(&err));
}

#[test]
fn deleted_endpoint_is_unavailable() {
    assert!(is_unavailable_endpoint(&invalid_parameter(
        "Invalid parameter: TargetArn Reason: No endpoint found for the target arn specified",
    )));
    assert!(is_unavailable_endpoint(&PublishError::NotFoundException(
        NotFoundException::builder()
            .message("Endpoint does not exist")
            .build(),
    )));
}

#[test]
fn message_and_platform_errors_are_not_unavailable() {
    assert!(!is_unavailable_endpoint(&invalid_parameter(
        "Invalid parameter: Message Reason: Invalid notification for protocol APNS: Notification is too long",
    )));
    assert!(!is_unavailable_endpoint(
        &PublishError::PlatformApplicationDisabledException(
            PlatformApplicationDisabledException::builder()
                .message("Platform application is disabled")
                .build(),
        )
    ));
}

#[tokio::test]
async fn unconfigured_mobile_delivery_returns_configuration_error() {
    let adapter = OptionalMobilePushAdapter::<aws_sdk_sns::Client>(None);
    let attributes = MessageAttributes {
        push_type: PushType::Background,
        collapse_key: "test".into(),
    };
    let ios = APNSPushNotification {
        aps: crate::domain::models::apple::Aps::default(),
        push_notification_data: (),
    };
    let android = FCMMessage::clear(
        "test".into(),
        (),
        macro_user_id::user_id::MacroUserIdStr::try_from_email("test@example.com").unwrap(),
    );
    for result in [
        adapter
            .send_ios_push_notification("arn:existing", &ios, &attributes)
            .await,
        adapter
            .send_android_push_notification("arn:existing", &android, &attributes)
            .await,
    ] {
        assert!(
            result
                .unwrap_err()
                .downcast_current_context::<crate::domain::models::mobile::MobilePushUnconfigured>()
                .is_some()
        );
    }
}

struct TestPushService;

impl MobilePushOps for TestPushService {
    async fn push_notification<T: Serialize + Send + Sync>(
        &self,
        _endpoint_arn: &str,
        message: &SnsTarget<'_, T>,
        attributes: HashMap<String, MessageAttributeValue>,
    ) -> Result<String, Report> {
        if matches!(message, SnsTarget::Ios(_)) {
            assert_eq!(
                attributes["AWS.SNS.MOBILE.APNS.TOPIC"].string_value(),
                Some("com.example.test")
            );
        }
        Ok("published-message-id".into())
    }
}

#[tokio::test]
async fn configured_mobile_adapter_keeps_sns_delivery() {
    let adapter = OptionalMobilePushAdapter(Some(MobilePushAdapter {
        push_service: TestPushService,
        apns_bundle_id: "com.example.test".into(),
        voip_bundle_id: None,
    }));
    let attributes = MessageAttributes {
        push_type: PushType::Background,
        collapse_key: "test".into(),
    };
    let ios = APNSPushNotification {
        aps: crate::domain::models::apple::Aps::default(),
        push_notification_data: (),
    };
    let android = FCMMessage::clear(
        "test".into(),
        (),
        macro_user_id::user_id::MacroUserIdStr::try_from_email("test@example.com").unwrap(),
    );
    assert_eq!(
        adapter
            .send_ios_push_notification("arn:ios", &ios, &attributes)
            .await
            .unwrap(),
        "published-message-id"
    );
    assert_eq!(
        adapter
            .send_android_push_notification("arn:android", &android, &attributes)
            .await
            .unwrap(),
        "published-message-id"
    );
}
