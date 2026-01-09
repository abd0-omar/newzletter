use axum::{body::to_bytes, http, response::Response};
use chrono::Utc;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

use super::IdempotencyKey;

pub async fn get_saved_response(
    pool: &SqlitePool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<Option<Response<Vec<u8>>>, anyhow::Error> {
    let user_id = user_id.to_string();
    let idempotency_key = idempotency_key.as_ref().to_string();
    let saved_response = sqlx::query!(
        r#"
            SELECT
                response_status_code as "response_status_code!",
                response_headers as "response_headers!",
                response_body as "response_body!: Vec<u8>"
            FROM idempotency
            WHERE
                user_uuid = $1 AND
                idempotency_key = $2
        "#,
        user_id,
        idempotency_key,
    )
    .fetch_optional(pool)
    .await?;

    match saved_response {
        Some(r) => {
            let status_code = StatusCode::from_u16(r.response_status_code.try_into()?)?;

            let mut response = Response::builder()
                .status(status_code)
                .body(r.response_body)?;

            let response_headers: Vec<HeaderPair> = serde_json::from_str(&r.response_headers)?;

            for HeaderPair { name, value } in response_headers {
                let name = http::HeaderName::from_bytes(name.as_bytes())?;
                let value = http::HeaderValue::from_bytes(&value)?;
                response.headers_mut().append(name, value);
            }

            Ok(Some(response))
        }
        None => Ok(None),
    }
}

#[derive(Deserialize, Serialize)]
struct HeaderPair {
    name: String,
    value: Vec<u8>,
}

pub async fn save_response(
    mut transaction: Transaction<'static, Sqlite>,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
    http_response: axum::response::Response,
) -> Result<Response, anyhow::Error> {
    let (response_head, body) = http_response.into_parts();
    let body = to_bytes(body, usize::MAX).await?;

    let status_code = response_head.status.as_u16() as i16;

    let headers = {
        let mut h = Vec::with_capacity(response_head.headers.len());
        for (name, value) in response_head.headers.iter() {
            let name = name.as_str().to_owned();
            let value = value.as_bytes().to_owned();
            let header_pair = HeaderPair { name, value };
            h.push(header_pair);
        }
        h
    };

    let user_id = user_id.to_string();
    let idempotency_key = idempotency_key.as_ref().to_string();
    let headers = serde_json::to_string(&headers)?;
    let body = body.to_vec();

    sqlx::query!(
        r#"
            UPDATE IDEMPOTENCY
            SET
                response_status_code = $3,
                response_headers = $4,
                response_body = $5
            WHERE
                user_uuid = $1 AND
                idempotency_key = $2
        "#,
        user_id,
        idempotency_key,
        status_code,
        headers,
        body
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    let http_response = axum::response::Response::from_parts(response_head, body);
    Ok(http_response.map(axum::body::Body::from))
}

pub enum NextAction {
    ReturnSavedResponse(Response),
    StartProcessing(Transaction<'static, Sqlite>),
}

pub async fn try_processing(
    pool: &SqlitePool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<NextAction, anyhow::Error> {
    let mut transaction = pool.begin().await?;
    let user_id_string = user_id.to_string();
    let idempotency_key_string = idempotency_key.as_ref().to_owned();
    let now = Utc::now().to_string();
    let n_inserted_rows = sqlx::query!(
        r#"
            INSERT INTO idempotency (
                user_uuid,
                idempotency_key,
                created_at
            )
            VALUES ($1, $2, $3)
            ON CONFLICT DO NOTHING
        "#,
        user_id_string,
        idempotency_key_string,
        now
    )
    .execute(&mut *transaction)
    .await?
    .rows_affected();

    if n_inserted_rows > 0 {
        Ok(NextAction::StartProcessing(transaction))
    } else {
        let saved_response = get_saved_response(pool, &idempotency_key, user_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("We expected a saved response, we didn't find it"))?;

        Ok(NextAction::ReturnSavedResponse(
            saved_response.map(axum::body::Body::from),
        ))
    }
}
