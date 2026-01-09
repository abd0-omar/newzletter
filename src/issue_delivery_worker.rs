use crate::configuration::{configure_database, Settings};
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use sqlx::SqlitePool;
use std::time::Duration;
use tracing::{field::display, Span};
use uuid::Uuid;

pub async fn run_worker_until_stopped(configuration: Settings) -> Result<(), anyhow::Error> {
    let connection_pool = configure_database(&configuration.database).await?;
    let email_client = configuration.email_client.client();
    worker_loop(connection_pool, email_client).await
}

async fn worker_loop(pool: SqlitePool, email_client: EmailClient) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pool, &email_client).await {
            Ok(ExecutionOutcome::EmptyQueue) => {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(ExecutionOutcome::TaskCompleted) => {}
        }
    }
}

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

#[tracing::instrument(
    skip_all,
    fields(
        newsletter_issue_id=tracing::field::Empty,
        subscriber_email=tracing::field::Empty
    ),
    err
)]
pub async fn try_execute_task(
    pool: &SqlitePool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, anyhow::Error> {
    let task = dequeue_task(pool).await?;
    if task.is_none() {
        return Ok(ExecutionOutcome::EmptyQueue);
    }
    let (issue_id, email) = task.unwrap();
    Span::current()
        .record("newsletter_issue_id", display(issue_id))
        .record("subscriber_email", display(&email));
    match SubscriberEmail::parse(email.clone()) {
        Ok(email) => {
            let issue = get_issue(pool, &issue_id).await?;
            if let Err(e) = email_client
                .send_email(
                    &email,
                    &issue.title,
                    &issue.html_content,
                    &issue.text_content,
                )
                .await
            {
                tracing::error!(
                    error.cause_chain = ?e,
                    error.message = %e,
                    "Failed to deliver issue to a confirmed subscriber. \
                        Skipping.",
                );
            }
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "Skipping a confirmed subscriber. \
                    Their stored contact details are invalid",
            );
        }
    }
    Ok(ExecutionOutcome::TaskCompleted)
}

#[tracing::instrument(skip_all)]
async fn dequeue_task(pool: &SqlitePool) -> Result<Option<(Uuid, String)>, anyhow::Error> {
    let r = sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE rowid IN (
            SELECT rowid
            FROM issue_delivery_queue
            LIMIT 1
        )
        RETURNING newsletter_issue_uuid, subscriber_email
        "#,
    )
    .fetch_optional(pool)
    .await?;
    if let Some(r) = r {
        let issue_id = Uuid::parse_str(&r.newsletter_issue_uuid)?;
        Ok(Some((issue_id, r.subscriber_email)))
    } else {
        Ok(None)
    }
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(skip_all)]
async fn get_issue(pool: &SqlitePool, issue_id: &Uuid) -> Result<NewsletterIssue, anyhow::Error> {
    let issue_id_string = issue_id.to_string();
    let issue = sqlx::query_as!(
        NewsletterIssue,
        r#"
        SELECT title, text_content, html_content
        FROM newsletter_issues
        WHERE
            newsletter_issue_uuid = $1
        "#,
        issue_id_string
    )
    .fetch_one(pool)
    .await?;
    Ok(issue)
}
