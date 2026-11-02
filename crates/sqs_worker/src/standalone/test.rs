use super::*;

#[test]
fn sqs_keeps_attributes_used_by_handlers() {
    let message = Message::builder()
        .body("{}")
        .message_id("message-1")
        .message_attributes(
            "chat_id",
            aws_sdk_sqs::types::MessageAttributeValue::builder()
                .data_type("String")
                .string_value("chat-1")
                .build()
                .unwrap(),
        )
        .message_attributes(
            "bytes",
            aws_sdk_sqs::types::MessageAttributeValue::builder()
                .data_type("Binary")
                .binary_value(aws_sdk_sqs::primitives::Blob::new([0, 255]))
                .build()
                .unwrap(),
        )
        .attributes(
            aws_sdk_sqs::types::MessageSystemAttributeName::ApproximateReceiveCount,
            "3",
        )
        .build();
    let events = payloads(&message, EventFormat::Sqs).unwrap();
    let decoded: aws_lambda_events::sqs::SqsEvent =
        serde_json::from_value(events[0].clone()).unwrap();
    assert_eq!(
        decoded.records[0].message_attributes["chat_id"]
            .string_value
            .as_deref(),
        Some("chat-1")
    );
    assert_eq!(
        events[0]["Records"][0]["messageAttributes"]["chat_id"]["stringValue"],
        "chat-1"
    );
    assert_eq!(
        events[0]["Records"][0]["messageAttributes"]["bytes"]["binaryValue"],
        "AP8="
    );
    assert_eq!(
        events[0]["Records"][0]["attributes"]["ApproximateReceiveCount"],
        "3"
    );
}

#[test]
fn s3_notifications_become_individual_eventbridge_payloads() {
    let message = Message::builder().body(r#"{"Records":[{"s3":{"bucket":{"name":"bucket"},"object":{"key":"a+b%2Bc.pdf"}}},{"s3":{"bucket":{"name":"bucket"},"object":{"key":"second.pdf"}}}]}"#).build();
    let events = payloads(&message, EventFormat::EventBridgeS3).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["detail"]["object"]["key"], "a b+c.pdf");
    assert_eq!(events[1]["detail"]["object"]["key"], "second.pdf");
}

#[test]
fn eventbridge_keys_are_not_decoded_twice() {
    let message = Message::builder().body(r#"{"source":"aws.s3","detail":{"bucket":{"name":"bucket"},"object":{"key":"a+b%20.pdf"}}}"#).build();
    let events = payloads(&message, EventFormat::EventBridgeS3).unwrap();
    assert_eq!(events[0]["detail"]["object"]["key"], "a+b%20.pdf");
}

#[test]
fn s3_test_events_are_acknowledged_without_invoking_a_handler() {
    let message = Message::builder()
        .body(r#"{"Event":"s3:TestEvent"}"#)
        .build();
    assert!(payloads(&message, EventFormat::S3).unwrap().is_empty());
    assert!(
        payloads(&message, EventFormat::EventBridgeS3)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn sns_envelope_preserves_the_ses_notification_string() {
    let message = Message::builder().body(r#"{"Type":"Notification","MessageId":"id","TopicArn":"arn:aws:sns:us-east-2:000000000000:ses","Subject":"SES","Timestamp":"2026-11-02T00:00:00Z","SignatureVersion":"1","Signature":"signature","SigningCertURL":"https://example.com/cert","UnsubscribeURL":"https://example.com/unsubscribe","Message":"{\"notificationType\":\"Bounce\"}"}"#).build();
    let events = payloads(&message, EventFormat::Sns).unwrap();
    assert_eq!(
        events[0]["Records"][0]["Sns"]["Message"],
        r#"{"notificationType":"Bounce"}"#
    );
    let _: aws_lambda_events::sns::SnsEvent = serde_json::from_value(events[0].clone()).unwrap();
    let raw = Message::builder()
        .body(r#"{"notificationType":"Bounce"}"#)
        .build();
    assert!(payloads(&raw, EventFormat::Sns).is_err());
}

#[tokio::test]
async fn one_shot_failure_reaches_the_process() {
    let result = run_once(
        |_: LambdaEvent<Value>| async { Err("retry required".into()) },
        scheduled_event(),
    )
    .await;
    assert_eq!(result.unwrap_err().to_string(), "retry required");
}

#[test]
fn invocation_has_a_real_deadline_for_retention() {
    let event = invocation(json!({})).unwrap();
    assert!(
        event
            .context
            .deadline()
            .duration_since(SystemTime::now())
            .unwrap()
            > Duration::from_secs(890)
    );
    assert_eq!(deletion_event(), json!({"dry_run":false}));
}
