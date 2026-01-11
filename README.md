# newzletter

A production-ready newsletter service built in Rust, following [Zero to Production in Rust](https://www.zero2prod.com/).

## Architecture

This project adapts the "Zero to Production in Rust" book's Actix-Web + PostgreSQL stack to a more cost-effective setup:

| Book | This Project |
|------|--------------|
| Actix-Web | **Axum** |
| PostgreSQL | **SQLite** |
| Postgres on cloud | **Litestream** (SQLite → S3 replication) |
| Digital Ocean | **Fly.io** (auto-stop for free tier) |

### Why This Stack?

- **SQLite + Litestream**: Replicate/backup SQLite to an S3 bucket. Fly.io waives costs under $5/month
- **Axum**: Modern, tower-based web framework with great ergonomics
- **Fly.io**: Auto-stop machines when idle, auto-start on request - perfect for low-traffic apps

## Implemented Features

### Core Newsletter Functionality

- **Subscription System**
  - Email subscription with form validation
  - Double opt-in via confirmation emails
  - Subscription tokens for secure confirmation
  - Status tracking (pending → confirmed)

- **Newsletter Publishing**
  - Admin-only newsletter composition
  - HTML and plain text content support
  - Bulk delivery to confirmed subscribers

### Background Workers

Following Chapter 10's patterns for reliable email delivery:

```rust
// Main spawns both the API server and background worker
let application_task = tokio::spawn(application.run_until_stopped());
let worker_task = tokio::spawn(run_worker_until_stopped(configuration));

tokio::select! {
    o = application_task => { report_exit("API", o) }
    o = worker_task => { report_exit("Background worker", o) }
};
```

- **Issue Delivery Queue**: Newsletter issues are queued for async delivery
- **Worker Loop**: Continuously polls for pending deliveries
- **Graceful Degradation**: Failed deliveries are logged, queue continues processing
- **Backoff Strategy**: Sleeps on empty queue or errors to prevent busy-waiting

### Idempotency

Robust handling of duplicate POST requests (Chapter 10):

```rust
pub enum NextAction {
    ReturnSavedResponse(Response),
    StartProcessing(Transaction<'static, Sqlite>),
}
```

- **Idempotency Keys**: Client-provided keys prevent duplicate newsletter sends
- **Response Caching**: Saves full HTTP response (status, headers, body) for replay
- **Transaction Safety**: Uses `INSERT ... ON CONFLICT DO NOTHING` pattern
- **Atomic Operations**: Either starts processing or returns cached response

### Database Transactions

Safe, atomic operations throughout:

- **Subscription Flow**: Insert subscriber + store token in single transaction
- **Newsletter Publishing**: Insert issue + enqueue deliveries atomically
- **Idempotency**: Transaction spans entire request lifecycle

```rust
// Example: Newsletter publishing with transaction
let mut transaction = pool.begin().await?;
let issue_id = insert_newsletter_issue(&mut transaction, ...).await?;
enqueue_delivery_tasks(&mut transaction, issue_id).await?;
// Response is saved and transaction committed together
save_response(transaction, &idempotency_key, user_id, response).await?;
```

### Authentication & Authorization

Session-based authentication with Redis:

- **Password Hashing**: Argon2id with secure parameters
- **Session Management**: Redis-backed sessions with `tower-sessions`
- **Auth Middleware**: Protects admin routes, redirects anonymous users
- **Password Change**: Secure password update flow

```rust
// Middleware rejects anonymous users on admin routes
.layer(middleware::from_fn(reject_anonymous_users))
```

### Telemetry & Observability

Structured logging following Chapter 4's patterns:

- **Tracing**: Request spans with method, URI, request ID
- **Bunyan Formatter**: JSON-structured logs for production
- **Span Context**: Propagates trace context to blocking tasks
- **Error Chains**: Formats full error cause chains for debugging

```rust
// Every request gets a unique ID and timing
info_span!(
    "http_request",
    method = ?request.method(),
    uri = ?request.uri(),
    request_id = ?Uuid::new_v4(),
)
```

### Domain-Driven Design

Type-safe domain modeling:

- **`SubscriberEmail`**: Validated email with `validator` crate
- **`SubscriberName`**: Unicode-aware validation (grapheme clusters, forbidden chars)
- **`NewSubscriber`**: Aggregate for subscription data
- **`IdempotencyKey`**: Newtype for request deduplication

### Error Handling

Comprehensive error strategy:

- **`thiserror`**: Derive Error for custom types
- **`anyhow`**: Application-level error handling
- **Error Chains**: Full cause chain formatting
- **HTTP Mapping**: Errors map to appropriate status codes

```rust
#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}
```

### Configuration

Layered configuration system:

```
configuration/
├── base.yaml      # Shared settings
├── local.yaml     # Development overrides
└── production.yaml # Production settings
```

- **Environment Detection**: `APP_ENVIRONMENT` switches configs
- **Env Var Overrides**: `APP_APPLICATION__PORT=5001` pattern
- **SQLite Tuning**: WAL mode, MMAP, cache size, etc.

### Testing

Comprehensive test suite:

- **Integration Tests**: Full HTTP request/response testing
- **Mock Email Server**: `wiremock` for email API simulation
- **Test Isolation**: Each test gets unique SQLite database
- **Property Testing**: `quickcheck` for email validation
- **Helpers**: `TestApp` struct with convenience methods

```rust
// Tests can dispatch pending emails synchronously
pub async fn dispatch_all_pending_emails(&self) {
    loop {
        if let ExecutionOutcome::EmptyQueue =
            try_execute_task(&self.db_pool, &self.email_client).await.unwrap()
        {
            break;
        }
    }
}
```

### Email Client

Postmark API integration:

- **Configurable Timeouts**: Prevent hanging on slow responses
- **Authorization**: Secure token handling with `secrecy`
- **Error Handling**: Proper status code checking

## Project Structure

```
src/
├── authentication/     # Login, password, middleware
├── domain/            # SubscriberEmail, SubscriberName, NewSubscriber
├── idempotency/       # Key validation, response persistence
├── routes/
│   ├── admin/         # Dashboard, newsletter, password
│   ├── login/         # Login form and handler
│   └── subscriptions/ # Subscribe and confirm
├── configuration.rs   # Settings and database setup
├── email_client.rs    # Postmark API client
├── issue_delivery_worker.rs  # Background email delivery
├── startup.rs         # Application bootstrap
└── telemetry.rs       # Tracing setup
```

## Deployment

### Fly.io Configuration

```toml
app = 'newzletter'
primary_region = 'ewr'  # Cheapest region

[[mounts]]
source = 'data'
destination = '/app/data'

[http_service]
auto_stop_machines = 'stop'   # Stop when idle
auto_start_machines = true    # Wake on request
min_machines_running = 0      # Allow full stop

[[vm]]
memory = '512mb'
cpu_kind = 'shared'
cpus = 1
```

### Litestream

SQLite replication to S3 for durability:

```yaml
# etc/litestream.yml
dbs:
  - path: /app/data/newzletter.db
    replicas:
      - url: s3://bucket/newzletter
```

## Running Locally

```bash
# Start Redis
./scripts/init_redis.sh

# Run the application
cargo run

# Run tests
cargo test
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `APP_ENVIRONMENT` | `local` or `production` |
| `APP_APPLICATION__PORT` | Override port |
| `APP_DATABASE__DATABASE_PATH` | SQLite file path |
| `APP_REDIS_URI` | Redis connection string |
| `APP_EMAIL_CLIENT__AUTHORIZATION_TOKEN` | Postmark API token |

## Key Dependencies

- **axum**: Web framework
- **sqlx**: Async SQL with compile-time checks
- **tower-sessions**: Session management
- **tower-sessions-redis-store**: Redis session backend
- **argon2**: Password hashing
- **tracing** + **tracing-bunyan-formatter**: Structured logging
- **reqwest**: HTTP client for email API
- **secrecy**: Sensitive data handling
- **Astro + rinja**: HTML templating

## Credits

Based on [Zero to Production in Rust](https://www.zero2prod.com/) by Luca Palmieri. Adapted for Axum + SQLite + Fly.io.
