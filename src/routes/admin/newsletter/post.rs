use crate::authentication::UserId;
use crate::idempotency::{save_response, try_processing, IdempotencyKey};
use crate::startup::AppState;
use crate::utils::{e400, e500};
use anyhow::Context;
use axum::extract::State;
use axum::response::{IntoResponse, Redirect};
use axum::{Extension, Form};
use axum_messages::Messages;
use chrono::Utc;
use sqlx::{Sqlite, Transaction};
use std::sync::Arc;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Sqlite>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_uuid = Uuid::new_v4();
    let newsletter_issue_uuid_string = newsletter_issue_uuid.to_string();
    let now = Utc::now().to_string();

    sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_uuid, 
            title, 
            text_content, 
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, $5)
        "#,
        newsletter_issue_uuid_string,
        title,
        text_content,
        html_content,
        now
    )
    .execute(&mut **transaction)
    .await?;

    Ok(newsletter_issue_uuid)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Sqlite>,
    newsletter_issue_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    let newsletter_issue_uuid_string = newsletter_issue_uuid.to_string();

    sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_uuid, 
            subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_uuid_string,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(form, app_state, messages, user_id),
    fields(user_id=%user_id),
)]
pub async fn publish_newsletter(
    State(app_state): State<Arc<AppState>>,
    messages: Messages,
    Extension(user_id): Extension<UserId>,
    Form(form): Form<FormData>,
) -> Result<axum::response::Response, axum::response::Response> {
    let idempotency_key: IdempotencyKey = form.idempotency_key.try_into().map_err(e400)?;

    let mut transaction = match try_processing(&app_state.pool, &idempotency_key, *user_id)
        .await
        .map_err(e500)?
    {
        crate::idempotency::NextAction::StartProcessing(transaction) => transaction,
        crate::idempotency::NextAction::ReturnSavedResponse(saved_response) => {
            messages.info("The newsletter issue has been published!");
            return Ok(saved_response);
        }
    };

    let issue_id = insert_newsletter_issue(
        &mut transaction,
        &form.title,
        &form.text_content,
        &form.html_content,
    )
    .await
    .context("Failed to store newsletter issue details")
    .map_err(e500)?;

    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(e500)?;

    messages.info("The newsletter issue has been published!");

    let response = Redirect::to("/admin/newsletters").into_response();
    let response = save_response(transaction, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;

    return Ok(response);
}
