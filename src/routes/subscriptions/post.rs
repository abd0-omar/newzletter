use std::sync::Arc;

use anyhow::Context;
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use chrono::Utc;
use rand::{distr::Alphanumeric, rng, Rng};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use sqlx::{Sqlite, Transaction};
use uuid::Uuid;

use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::AppState,
};

#[derive(Deserialize)]
pub struct FormData {
    name: String,
    email: String,
    #[serde(rename = "cf-turnstile-response")]
    cf_turnstile_response: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;
        Ok(Self { name, email })
    }
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error("Turnstile verification failed")]
    TurnstileError,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl IntoResponse for SubscribeError {
    fn into_response(self) -> axum::response::Response {
        match self {
            SubscribeError::ValidationError(e) => {
                tracing::error!(cause_chain = ?e);
                Redirect::to("/?error=validation").into_response()
            }
            SubscribeError::TurnstileError => {
                tracing::error!("Turnstile verification failed");
                Redirect::to("/?error=captcha").into_response()
            }
            SubscribeError::UnexpectedError(e) => {
                tracing::error!(cause_chain = ?e);
                Redirect::to("/?error=server").into_response()
            }
        }
    }
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, app_state),
    fields(
        subscriber_name = %form.name,
        subscriber_email = %form.email
    )
)]
pub async fn subscribe(
    State(app_state): State<Arc<AppState>>,
    Form(form): Form<FormData>,
) -> Result<impl IntoResponse, SubscribeError> {
    // Verify Turnstile token first
    verify_turnstile(&app_state.turnstile_secret, &form.cf_turnstile_response)
        .await
        .map_err(|_| SubscribeError::TurnstileError)?;

    let new_subscriber = form.try_into().map_err(SubscribeError::ValidationError)?;
    let mut transaction = app_state
        .pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;

    // Try to insert subscriber - if email already exists, just redirect to success
    // (don't leak information about who's subscribed)
    let subscriber_id = match insert_subscriber(&mut transaction, &new_subscriber).await {
        Ok(id) => id,
        Err(e) => {
            // Check if it's a UNIQUE constraint error (duplicate email)
            if e.to_string().contains("UNIQUE constraint failed") {
                tracing::info!("Email already subscribed, redirecting to success");
                return Ok(Redirect::to("/?subscribed=true"));
            }
            return Err(anyhow::anyhow!("Failed to insert new subscriber: {}", e).into());
        }
    };

    let subscription_token = generate_subscription_token();
    store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .context("Failed to store the confirmation token for a new subscriber.")?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new subscriber.")?;
    send_confirmation_email(
        &app_state.email_client,
        new_subscriber,
        &app_state.base_url.0,
        &subscription_token,
    )
    .await
    .context("Failed to send a confirmation email.")?;

    Ok(Redirect::to("/?subscribed=true"))
}

fn generate_subscription_token() -> String {
    let mut rng = rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[derive(Deserialize)]
struct TurnstileResponse {
    success: bool,
}

#[tracing::instrument(name = "Verifying Turnstile token", skip(secret, response_token))]
async fn verify_turnstile(
    secret: &SecretString,
    response_token: &str,
) -> Result<(), anyhow::Error> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&[
            ("secret", secret.expose_secret()),
            ("response", response_token),
        ])
        .send()
        .await
        .context("Failed to send Turnstile verification request")?;

    let turnstile_response: TurnstileResponse = response
        .json()
        .await
        .context("Failed to parse Turnstile response")?;

    if turnstile_response.success {
        tracing::info!("Turnstile verification successful");
        Ok(())
    } else {
        tracing::warn!("Turnstile verification failed");
        anyhow::bail!("Turnstile verification failed")
    }
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber, base_url, subscription_token)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );
    let plain_body = format!(
        "Willkommen zu unserem newzletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    let html_body = format!(
        "Willkommen zu unserem newzletter!<br />Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );
    email_client
        .send_email(
            &new_subscriber.email,
            "Willkommen!",
            &html_body,
            &plain_body,
        )
        .await
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Sqlite>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let uuid = Uuid::new_v4();
    let subscriber_id = uuid.to_string();
    let timestamptz = Utc::now().to_string();
    let name = new_subscriber.name.as_ref();
    let email = new_subscriber.email.as_ref();
    sqlx::query!(
        r#"
            INSERT INTO subscriptions(uuid, name, email, subscribed_at, status) VALUES($1, $2, $3, $4, 'pending_confirmation')
            "#,
        subscriber_id,
        name,
        email,
        timestamptz,
    ).execute(&mut **transaction).await?;
    Ok(uuid)
}

#[tracing::instrument(
    name = "Store subscription token in the database",
    skip(subscription_token, transaction)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Sqlite>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    let subscriber_id = subscriber_id.to_string();
    sqlx::query!(
        r#"
    INSERT INTO subscription_tokens (subscription_token, subscriber_id)
    VALUES ($1, $2)
        "#,
        subscription_token,
        subscriber_id
    )
    .execute(&mut **transaction)
    .await
    .map_err(StoreTokenError)?;
    Ok(())
}

pub struct StoreTokenError(sqlx::Error);

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database failure was encountered while trying to store a subscription token."
        )
    }
}

pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
