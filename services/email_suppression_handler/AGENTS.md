# email_suppression_handler

## Start here

`src/main.rs` wires the Lambda runtime. `src/handler.rs` owns processing.

## VM execution

`email_suppression_handler_vm` in `src/vm.rs` shares the Lambda handler. Set `VM_QUEUE_URL` to its dedicated SQS queue. The shared poller owns visibility renewal, acknowledgement and shutdown. Failures remain on the queue for redrive.

## Checks

`cargo test -p email_suppression_handler` with `SQLX_OFFLINE` unset.
