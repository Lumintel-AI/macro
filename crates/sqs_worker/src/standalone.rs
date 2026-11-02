//! VM transports for existing Lambda handlers. No Runtime API or Lambda service is used.

use std::{
    future::Future,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aws_sdk_sqs::types::Message;
use base64::{Engine, engine::general_purpose::STANDARD};
use lambda_runtime::{Context, Error, LambdaEvent};
use macro_env_var::env_vars;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tracing::Instrument;

use crate::SQSWorker;

env_vars! {
    struct VmQueueUrl;
}

const INVOCATION_TIMEOUT: Duration = Duration::from_secs(900);
const VISIBILITY_SECONDS: i32 = 300;
const HEARTBEAT: Duration = Duration::from_secs(60);

/// Wire format delivered by a dedicated queue.
#[derive(Clone, Copy)]
pub enum EventFormat {
    /// One SQS record, including its message attributes.
    Sqs,
    /// S3 ObjectCreated notification records.
    S3,
    /// S3 ObjectCreated notifications converted to EventBridge details.
    EventBridgeS3,
    /// An SNS subscription with raw message delivery disabled.
    Sns,
}

/// Construct the scheduled event expected by the upstream scheduled handlers.
pub fn scheduled_event() -> Value {
    json!({
        "version": "0", "id": "systemd", "detail-type": "Scheduled Event",
        "source": "systemd", "account": "", "time": chrono::Utc::now(),
        "region": "", "resources": [], "detail": {}
    })
}

/// Explicitly request notification deletion. The upstream empty event is a dry run.
pub fn deletion_event() -> Value {
    json!({"dry_run": false})
}

fn invocation<T>(payload: T) -> Result<LambdaEvent<T>, Error> {
    let deadline = SystemTime::now().duration_since(UNIX_EPOCH)? + INVOCATION_TIMEOUT;
    let mut context = Context::default();
    context.deadline = u64::try_from(deadline.as_millis())?;
    context.request_id = format!("vm-{}-{}", std::process::id(), deadline.as_nanos());
    Ok(LambdaEvent::new(payload, context))
}

/// Invoke a scheduled handler once and preserve its failure as a process error.
pub async fn run_once<T, F, Fut>(mut handler: F, payload: Value) -> Result<(), Error>
where
    T: DeserializeOwned,
    F: FnMut(LambdaEvent<T>) -> Fut,
    Fut: Future<Output = Result<(), Error>>,
{
    let event = invocation(serde_json::from_value(payload)?)?;
    tokio::time::timeout(
        INVOCATION_TIMEOUT,
        handler(event).instrument(tracing::info_span!("vm.job.invoke")),
    )
    .await?
}

/// Long-poll one message at a time, renew its lease, and delete only on success.
/// SQS owns retry counts and DLQ redrive. Configure a redrive policy on every queue.
/// SIGTERM stops new receives and lets the active invocation finish within 15 minutes.
pub async fn run_queue<T, F, Fut>(mut handler: F, format: EventFormat) -> Result<(), Error>
where
    T: DeserializeOwned,
    F: FnMut(LambdaEvent<T>) -> Fut,
    Fut: Future<Output = Result<(), Error>>,
{
    let queue_url = VmQueueUrl::new()?;
    let config = macro_aws_config::get_macro_aws_config().await;
    let worker = SQSWorker::new(
        aws_sdk_sqs::Client::new(&config),
        queue_url.to_string(),
        1,
        20,
    );
    let shutdown = macro_entrypoint::shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        let messages = tokio::select! {
            _ = &mut shutdown => return Ok(()),
            result = worker.receive_messages() => result?,
        };
        for message in messages {
            // Renew before parsing too, so malformed messages follow the same retry delay.
            worker
                .extend_visibility(&message, VISIBILITY_SECONDS)
                .await?;
            let processing = async {
                for payload in payloads(&message, format)? {
                    let event = invocation(serde_json::from_value(payload)?)?;
                    handler(event).await?;
                }
                Ok::<(), Error>(())
            };
            let span = tracing::info_span!(
                "vm.queue.invoke",
                message_id = ?message.message_id(),
                receive_count = ?message.attributes().and_then(|attributes| attributes.get(&aws_sdk_sqs::types::MessageSystemAttributeName::ApproximateReceiveCount)),
                upstream_traceparent = message.message_attributes().and_then(|attributes| attributes.get("traceparent")).and_then(|attribute| attribute.string_value()),
            );
            let processing = tokio::time::timeout(INVOCATION_TIMEOUT, processing.instrument(span));
            tokio::pin!(processing);
            let mut heartbeat =
                tokio::time::interval_at(tokio::time::Instant::now() + HEARTBEAT, HEARTBEAT);
            let result = loop {
                tokio::select! {
                    result = &mut processing => break result,
                    _ = heartbeat.tick() => worker.extend_visibility(&message, VISIBILITY_SECONDS).await?,
                }
            };
            match result {
                Ok(Ok(())) => worker.cleanup_message(&message).await?,
                Ok(Err(error)) => {
                    tracing::error!(error = ?error, message_id = ?message.message_id, "VM handler failed; retaining message for SQS retry")
                }
                Err(error) => {
                    tracing::error!(error = ?error, message_id = ?message.message_id, "VM handler timed out; retaining message for SQS retry")
                }
            }
        }
    }
}

fn payloads(message: &Message, format: EventFormat) -> Result<Vec<Value>, Error> {
    let body = message.body.as_deref().ok_or("SQS message has no body")?;
    match format {
        EventFormat::Sqs => {
            let attributes: serde_json::Map<String, Value> = message.message_attributes()
                .into_iter().flat_map(|attributes| attributes.iter()).map(|(name, attribute)| {
                    (name.clone(), json!({
                        "dataType": attribute.data_type(),
                        "stringValue": attribute.string_value(),
                        "binaryValue": attribute.binary_value().map(|bytes| STANDARD.encode(bytes.as_ref())),
                        "stringListValues": attribute.string_list_values(),
                        "binaryListValues": attribute.binary_list_values().iter().map(|bytes| STANDARD.encode(bytes.as_ref())).collect::<Vec<_>>()
                    }))
                }).collect();
            let system_attributes: serde_json::Map<String, Value> = message
                .attributes()
                .into_iter()
                .flat_map(|attributes| attributes.iter())
                .map(|(name, value)| (name.as_str().to_owned(), json!(value)))
                .collect();
            Ok(vec![json!({"Records": [{
                "messageId": message.message_id(), "receiptHandle": message.receipt_handle(),
                "body": body, "md5OfBody": message.md5_of_body(),
                "md5OfMessageAttributes": message.md5_of_message_attributes(),
                "attributes": system_attributes, "messageAttributes": attributes,
                "eventSource": "aws:sqs"
            }]})])
        }
        EventFormat::Sns => {
            let envelope: Value = serde_json::from_str(body)?;
            if envelope.get("Type").and_then(Value::as_str) != Some("Notification")
                || envelope.get("Message").and_then(Value::as_str).is_none()
            {
                return Err(
                    "expected SNS Notification envelope; disable raw message delivery".into(),
                );
            }
            Ok(vec![
                json!({"Records": [{"EventSource": "aws:sns", "EventVersion": "1.0", "EventSubscriptionArn": "", "Sns": envelope}]}),
            ])
        }
        EventFormat::S3 | EventFormat::EventBridgeS3 => {
            let event: Value = serde_json::from_str(body)?;
            // S3 sends a test notification when the bucket subscription is installed.
            if event.get("Event").and_then(Value::as_str) == Some("s3:TestEvent") {
                return Ok(vec![]);
            }
            if matches!(format, EventFormat::S3) {
                if !event.get("Records").is_some_and(Value::is_array) {
                    return Err("expected S3 notification Records".into());
                }
                return Ok(vec![event]);
            }
            // EventBridge rules may target the same dedicated SQS queue directly.
            if event.get("source").and_then(Value::as_str) == Some("aws.s3")
                && event.get("detail").is_some()
            {
                return Ok(vec![event]);
            }
            let records = event
                .get("Records")
                .and_then(Value::as_array)
                .ok_or("expected S3 notification Records")?;
            records
                .iter()
                .map(|record| {
                    let bucket = record
                        .pointer("/s3/bucket/name")
                        .and_then(Value::as_str)
                        .ok_or("missing S3 bucket")?;
                    let key = record
                        .pointer("/s3/object/key")
                        .and_then(Value::as_str)
                        .ok_or("missing S3 key")?;
                    // S3 notifications encode keys; EventBridge object keys are already decoded.
                    let encoded_key = key.replace('+', " ");
                    let key = urlencoding::decode(&encoded_key)?;
                    let mut converted = scheduled_event();
                    converted["source"] = json!("aws.s3");
                    converted["detail-type"] = json!("Object Created");
                    converted["detail"] =
                        json!({"bucket": {"name": bucket}, "object": {"key": key}});
                    Ok(converted)
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod test;
