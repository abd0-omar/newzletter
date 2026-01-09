use std::{fs, sync::LazyLock};

use argon2::{
    password_hash::{rand_core, PasswordHasher, SaltString},
    Algorithm, Argon2, Params, Version,
};
use newzletter::{
    configuration::{configure_database, get_configuration},
    issue_delivery_worker::try_execute_task,
    startup::Application,
    telemetry::{get_subscriber, init_subscriber},
};
use newzletter::{email_client::EmailClient, issue_delivery_worker::ExecutionOutcome};
use serde::Serialize;
use sqlx::sqlite::SqlitePool;
use tokio::fs::remove_file;
use uuid::Uuid;
use wiremock::MockServer;

// Ensure that the `tracing` stack is only initialised once using `once_cell`
static TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    };
});

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub db_pool: SqlitePool,
    pub email_server: MockServer,
    // to later delete it
    pub db_path: String,
    pub test_user: TestUser,
    pub api_client: reqwest::Client,
    pub email_client: EmailClient,
}

#[derive(Serialize)]
pub struct FormData {
    pub name: Option<String>,
    pub email: Option<String>,
}

pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

impl TestApp {
    pub async fn post_subscriptions(&self, form_data: &FormData) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/subscriptions", &self.address))
            .form(form_data)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/login", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_login_html(&self) -> String {
        self.api_client
            .get(&format!("{}/login", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
            .text()
            .await
            .unwrap()
    }

    pub async fn get_admin_dashboard_html(&self) -> String {
        self.get_admin_dashboard().await.text().await.unwrap()
    }

    pub async fn get_change_password(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/password", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_change_password_html(&self) -> String {
        self.get_change_password().await.text().await.unwrap()
    }

    pub async fn post_logout(&self) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/admin/logout", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_change_password<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/admin/password", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_publish_newsletter(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/newsletters", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_publish_newsletter_html(&self) -> String {
        self.get_publish_newsletter().await.text().await.unwrap()
    }

    pub async fn get_admin_dashboard(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/dashboard", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_publish_newsletter<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/admin/newsletters", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    /// Extract the confirmation links embedded in the request to the email API.
    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        // Extract the link from one of the request fields.
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            // Let's make sure we don't call random APIs on the web
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html = get_link(body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(body["TextBody"].as_str().unwrap());
        ConfirmationLinks { html, plain_text }
    }

    pub async fn dispatch_all_pending_emails(&self) {
        loop {
            if let ExecutionOutcome::EmptyQueue =
                try_execute_task(&self.db_pool, &self.email_client)
                    .await
                    .unwrap()
            {
                break;
            }
        }
    }

    pub async fn cleanup_test_db(&self) -> Result<(), sqlx::Error> {
        remove_file(&format!("{}.db", self.db_path)).await?;
        Ok(())
    }
}

pub async fn spawn_app() -> TestApp {
    // The first time `initialize` is invoked the code in `TRACING` is executed.
    // All other invocations will instead skip execution.
    LazyLock::force(&TRACING);

    fs::create_dir_all("scripts/a_place_for_test_dbs_to_spawn_in_it,supposed_to_be_empty_cuz_tests_terminate_after_success_execution/").expect("Failed to create directory");

    let email_server = MockServer::start().await;

    let configuration = {
        let mut configuration = get_configuration().expect("Failed to read configuration");
        configuration.application.port = 0;
        configuration.database.database_path = format!("scripts/a_place_for_test_dbs_to_spawn_in_it,supposed_to_be_empty_cuz_tests_terminate_after_success_execution/{}", Uuid::new_v4().to_string());
        configuration.database.create_if_missing = true;
        configuration.database.journal_mode = "MEMORY".to_string();
        configuration.database.synchronous = "OFF".to_string();
        configuration.database.busy_timeout = 5;
        configuration.database.foreign_keys = true;
        configuration.database.auto_vacuum = "NONE".to_string();
        configuration.database.page_size = 4096;
        configuration.database.cache_size = "-10000".to_string();
        configuration.database.mmap_size = "0".to_string();
        configuration.database.temp_store = "MEMORY".to_string();
        configuration.email_client.base_url = email_server.uri();
        configuration
    };

    let db_pool = configure_database(&configuration.database)
        .await
        .expect("Failed to configure database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");

    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application");

    let db_path = configuration.database.database_path;
    let application_host = configuration.application.host;
    let application_port = application.port();

    let address = format!("http://{}:{}", application_host, application.port());

    tokio::spawn(async move { application.run_until_stopped().await.unwrap() });

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let test_app = TestApp {
        address,
        port: application_port,
        db_pool: db_pool.clone(),
        db_path,
        email_server,
        test_user: TestUser::generate(),
        api_client: client,
        email_client: configuration.email_client.client(),
    };

    test_app.test_user.store(&db_pool).await;

    test_app
}

pub struct TestUser {
    uuid: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    pub async fn login(&self, app: &TestApp) {
        app.post_login(&serde_json::json!({
            "username": &self.username,
            "password": &self.password
        }))
        .await;
    }

    async fn store(&self, pool: &SqlitePool) {
        let salt = SaltString::generate(&mut rand_core::OsRng);

        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap(),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();

        let uuid = self.uuid.to_string();
        let username = self.username.to_string();
        let hashed_password = password_hash;

        sqlx::query!(
            "INSERT INTO users (uuid, username, password_hash)
            VALUES ($1, $2, $3)",
            uuid,
            username,
            hashed_password,
        )
        .execute(pool)
        .await
        .expect("Failed to store test user.");
    }
}

pub fn assert_is_redirect_to(response: &reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}
