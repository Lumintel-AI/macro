# user_link_cleanup_handler

## Start here

`src/main.rs` wires the Lambda runtime. `src/handler.rs` owns processing.

## VM execution

`user_link_cleanup_handler_vm` in `src/vm.rs` shares the Lambda handler. Run once per systemd timer. A handler failure exits nonzero. The timer must prevent overlapping runs.

## Checks

`cargo test -p user_link_cleanup_handler` with `SQLX_OFFLINE` unset.
