use crate::routes::subscribe_form;
use std::sync::Arc;

use axum::{
    extract::{FromRef, Request},
    middleware,
    response::Response,
    routing::{get, post},
    serve::Serve,
    Router,
};
use axum_messages::MessagesManagerLayer;
use secrecy::{ExposeSecret, SecretString};
use sqlx::SqlitePool;
use time::Duration;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_redis_store::{
    fred::{clients::Pool, prelude::*},
    RedisStore,
};

use crate::routes::{
    admin_dashboard, blog_index, blog_post, change_password, change_password_form, confirm,
    health_check, home, log_out, login, login_form, publish_newsletter, publish_newsletter_form,
    subscribe,
};
use crate::{
    authentication::reject_anonymous_users,
    configuration::{configure_database, Settings},
    email_client::EmailClient,
};
use tracing::{info, info_span, Span};
use uuid::Uuid;

pub struct AppState {
    pub pool: SqlitePool,
    pub email_client: EmailClient,
    pub base_url: ApplicationBaseUrl,
    _hmac_secret: HmacSecret,
}

// substate
impl FromRef<Arc<AppState>> for HmacSecret {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input._hmac_secret.clone()
    }
}

pub struct ApplicationBaseUrl(pub String);

pub async fn run(
    listener: TcpListener,
    pool: SqlitePool,
    email_client: EmailClient,
    base_url: String,
    _hmac_secret: SecretString,
    redis_uri: SecretString,
) -> anyhow::Result<Serve<TcpListener, Router, Router>> {
    // redis sessions
    let redis_url = redis_uri.expose_secret();
    let redis_config = Config::from_url(redis_url)
        .map_err(|e| anyhow::anyhow!("Failed to parse Redis URL: {}", e))?;

    let redis_pool = Pool::new(redis_config, None, None, None, 6)?;

    let _redis_conn = redis_pool.connect();
    redis_pool.wait_for_connect().await?;

    let session_store = RedisStore::new(redis_pool);
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::minutes(10)));

    let admin_routes = Router::new()
        .route("/dashboard", get(admin_dashboard))
        .route("/password", get(change_password_form).post(change_password))
        .route("/logout", post(log_out))
        .route(
            "/newsletters",
            get(publish_newsletter_form).post(publish_newsletter),
        )
        .layer(middleware::from_fn(reject_anonymous_users));

    // Wrapped in an Arc pointer to allow cheap cloning of AppState across handlers.
    // This prevents unnecessary cloning of EmailClient, which has two String fields,
    // since cloning an Arc is negligible.
    let app_state = Arc::new(AppState {
        pool,
        email_client,
        base_url: ApplicationBaseUrl(base_url),
        _hmac_secret: HmacSecret(SecretString::from(_hmac_secret)),
    });

    let app = Router::new()
        .route("/", get(home))
        .route("/login", get(login_form))
        .route("/login", post(login))
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscribe))
        .route("/subscriptions", get(subscribe_form))
        .route("/subscriptions/confirm", get(confirm))
        .route("/blog", get(blog_index))
        .route("/blog/{slug}", get(blog_post))
        .nest("/admin", admin_routes)
        .fallback_service(ServeDir::new("frontend/dist"))
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(|request: &Request<_>| {
                            let request_id = Uuid::new_v4();
                            info_span!(
                                "http_request",
                                method = ?request.method(),
                                uri = ?request.uri(),
                                version = ?request.version(),
                                request_id = ?request_id,
                            )
                        })
                        .on_response(
                            |response: &Response, latency: std::time::Duration, span: &Span| {
                                let status = response.status();
                                let headers = response.headers();
                                span.record("status", &status.as_u16());
                                info!(parent: span, ?status, ?headers, ?latency, "Response sent");
                            },
                        )
                        // By default `TraceLayer` will log 5xx responses but we're doing our specific
                        // logging of errors so disable that
                        .on_failure(()),
                )
                .layer(session_layer)
                .layer(MessagesManagerLayer),
        )
        .with_state(app_state);

    Ok(axum::serve(listener, app))
}

#[derive(Clone)]
pub struct HmacSecret(pub SecretString);

pub struct Application {
    port: u16,
    server: Serve<TcpListener, Router, Router>,
}

impl Application {
    // build is the one that invokes the `run()` function
    // then any fn invokes `run_until_stopped`
    pub async fn build(configuration: Settings) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        ))
        .await?;
        let port = listener.local_addr()?.port();

        let pool = configure_database(&configuration.database).await?;

        // let sender_email = configuration
        //     .email_client
        //     .sender()
        //     .expect("Invalid sender email address.");
        // let timeout = configuration.email_client.timeout();
        // let email_client = EmailClient::new(
        //     sender_email,
        //     configuration.email_client.base_url.clone(),
        //     configuration.email_client.authorization_token,
        //     timeout,
        // );
        let email_client = configuration.email_client.client();

        let server = run(
            listener,
            pool,
            email_client,
            configuration.application.base_url,
            configuration.application.hmac_secret,
            configuration.redis_uri,
        )
        .await?;

        Ok(Self { server, port })
    }

    pub async fn run_until_stopped(self) -> anyhow::Result<()> {
        Ok(self.server.await?)
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}
