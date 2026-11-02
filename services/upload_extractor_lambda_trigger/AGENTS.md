# upload_extractor_lambda_trigger

## Start here

`src/main.rs` wires the Lambda runtime. `src/handler.rs` owns processing.

## VM execution

`upload_extractor_lambda_trigger_vm` in `src/vm.rs` shares the Lambda handler. Set `VM_QUEUE_URL` to its dedicated SQS queue. The shared poller owns visibility renewal, acknowledgement and shutdown. Failures remain on the queue for redrive.

## Checks

`cargo test -p upload_extractor_lambda_trigger` with `SQLX_OFFLINE` unset.
