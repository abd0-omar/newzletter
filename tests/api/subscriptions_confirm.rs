use reqwest::StatusCode;
use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::{spawn_app, FormData};

#[tokio::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    // Arrange
    let app = spawn_app().await;
    // Act
    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();
    // Assert
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    app.cleanup_test_db().await.unwrap();
}

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    // Arrange
    let app = spawn_app().await;
    let fake_user_form_data = FormData {
        name: Some("abood".to_string()),
        email: Some("3la_el_7doood@yahoo.com".to_string()),
        cf_turnstile_response: Some("test-token".to_string()),
    };

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(&fake_user_form_data).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    // Act
    let response = reqwest::get(confirmation_links.html).await.unwrap();

    // Assert
    assert_eq!(response.status().as_u16(), 200);

    app.cleanup_test_db().await.unwrap();
}

#[tokio::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber() {
    // Arrange
    let app = spawn_app().await;
    let body = FormData {
        name: Some("abood".to_string()),
        email: Some("3la_el_7doood@yahoo.com".to_string()),
        cf_turnstile_response: Some("test-token".to_string()),
    };

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(&body).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    // Act
    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // Assert
    let saved = sqlx::query!("SELECT uuid, email, name, status FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.name, "abood");
    assert_eq!(saved.email, "3la_el_7doood@yahoo.com");
    assert_eq!(saved.status, "confirmed");

    app.cleanup_test_db().await.unwrap();
}
