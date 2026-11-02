# sqs_worker

## Start here

`src/lib.rs` wraps SQS polling, acknowledgement and visibility changes.
`src/standalone.rs` adapts Lambda handler payloads for plain VM processes.

## Checks

`cargo test -p sqs_worker` needs no database for its transport tests.

## Gotchas

- VM workers receive one message with all user and system attributes.
- Set `VM_QUEUE_URL` to a dedicated queue with a redrive policy and DLQ.
- Keep the container stop grace period above the 900-second invocation timeout.
- SNS subscriptions must disable raw message delivery.
- Domain policy stays in the supplied handler, outside this transport crate.
