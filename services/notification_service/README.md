# Notification Service

This service is responsible for processing and storing notifications.

## Local Database Setup

From the repository root:

```bash
just run_dbs -d
```

Now go to `services/notification_service`:

Set your DATABASE_URL environment variable:

```bash
cp ./.env.sample ./.env
```

Now that the database is running, you can run the following commands from `crates/notification_db_client`:
This will create the database and run any migrations.

```bash
sqlx database setup
```

To reset your local database, run the following commands:

```bash
sqlx database drop -y
```

## Optional mobile push

Production startup supports deployments without mobile applications. Leave
`APPLE_BUNDLE_ID`, `SNS_APNS_PLATFORM_ARN`, `SNS_FCM_PLATFORM_ARN` and
`SNS_APNS_VOIP_PLATFORM_ARN` absent or blank to disable mobile push. Set all four
to enable the existing SNS adapters. Partial configuration fails before the
service opens database or AWS connections.

Without mobile configuration, device registration returns HTTP 503 with
`mobile push is not configured`. Mobile delivery returns the same typed error
through the notification port. Existing gateway delivery and email digest
processing continue. Disabling mobile push does not delete saved devices.
