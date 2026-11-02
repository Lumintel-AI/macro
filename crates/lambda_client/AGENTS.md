# lambda_client

`Lambda` invokes upstream asynchronous callbacks. `job_update` adds the `JobUpdateSender` port, startup transport selection and outbound Lambda/gateway adapters for document jobs.

Read `src/job_update.rs` when changing callback delivery. `JOB_UPDATE_TRANSPORT` defaults to `lambda`; `connection_gateway` uses `ConnectionGatewayUrl` and the fleet key. Load configuration once at startup, preserve the job payload, and propagate failed deliveries before acknowledging a queue message.

Run `cargo test -p lambda_client` and `cargo test -p connection_gateway_client` for callback changes. Run the lock-pinned `cargo run -p xtask_deps -- --check` after changing its dependencies.
