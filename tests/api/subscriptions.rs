use reqwest::StatusCode;
use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::{spawn_app, FormData};

#[tokio::test]
async fn subscribe_returns_a_303_for_valid_form_data() {
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

    // Act
    let response = app.post_subscriptions(&fake_user_form_data).await;

    // Assert
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    app.cleanup_test_db().await.unwrap();
}

#[tokio::test]
async fn subscribe_persists_the_new_subscriber() {
    // Arrange
    let app = spawn_app().await;
    let fake_user_form_data = FormData {
        name: Some("abood".to_string()),
        email: Some("3la_el_7doood@yahoo.com".to_string()),
        cf_turnstile_response: Some("test-token".to_string()),
    };

    // Act
    app.post_subscriptions(&fake_user_form_data).await;

    // Assert
    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.name, "abood");
    assert_eq!(saved.email, "3la_el_7doood@yahoo.com");
    assert_eq!(saved.status, "pending_confirmation");

    app.cleanup_test_db().await.unwrap();
}

#[tokio::test]
async fn subscribe_fails_if_there_is_a_fatal_database_error() {
    // Arrange
    let app = spawn_app().await;
    let body = FormData {
        name: Some("abood".to_string()),
        email: Some("3la_el_7doood@yahoo.com".to_string()),
        cf_turnstile_response: Some("test-token".to_string()),
    };
    // Sabotage the database
    sqlx::query!("ALTER TABLE subscriptions RENAME TO broken_subscriptions;")
        .execute(&app.db_pool)
        .await
        .unwrap();
    // sqlx::query!("ALTER TABLE subscription_tokens RENAME TO broken_subscriptions;")
    //     .execute(&app.db_pool)
    //     .await
    //     .unwrap();

    // Act
    let response = app.post_subscriptions(&body).await;

    // Assert - errors now redirect back to home with error param
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get("Location").unwrap(),
        "/?error=server"
    );

    app.cleanup_test_db().await.unwrap();
}

#[tokio::test]
pub async fn subscribe_returns_a_422_when_data_is_missing() {
    // Arrange

    let app = spawn_app().await;

    let test_cases = vec![
        (
            FormData {
                name: Some("abood".to_string()),
                email: None,
                cf_turnstile_response: Some("test-token".to_string()),
            },
            "missing the email",
        ),
        (
            FormData {
                name: None,
                email: Some("email@email_proivderdotcom".to_string()),
                cf_turnstile_response: Some("test-token".to_string()),
            },
            "missing the name",
        ),
        (
            FormData {
                name: None,
                email: None,
                cf_turnstile_response: Some("test-token".to_string()),
            },
            "missing both",
        ),
    ];
    // Act
    for (invalid_form, error_message) in test_cases {
        let response = app.post_subscriptions(&invalid_form).await;
        // Assert
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "the API did not fail with 422 Bad Request when the payload was {}",
            error_message
        );
    }

    app.cleanup_test_db().await.unwrap();
}

#[tokio::test]
async fn subscribe_returns_a_400_when_fields_are_present_but_invalid() {
    // Arrange
    let app = spawn_app().await;
    let test_cases = [
        (
            FormData {
                name: Some("".to_string()),
                email: Some("hamada123@yahoo.com".to_string()),
                cf_turnstile_response: Some("test-token".to_string()),
            },
            "name present (gift) but empty",
        ),
        (
            FormData {
                name: Some("hamada".to_string()),
                email: Some("".to_string()),
                cf_turnstile_response: Some("test-token".to_string()),
            },
            "empty email",
        ),
        (
            FormData {
                name: Some("hamada".to_string()),
                email: Some("definitely-not-(blitzcrank)-an-email".to_string()),
                cf_turnstile_response: Some("test-token".to_string()),
            },
            "invalid email",
        ),
    ];

    for (form_data, description) in test_cases {
        // Act
        let response = app.post_subscriptions(&form_data).await;

        // Assert - validation errors now redirect back to home with error param
        assert_eq!(
            StatusCode::SEE_OTHER,
            response.status(),
            "The API did not return a 303 redirect when the payload was {}.",
            description
        );
        assert_eq!(
            response.headers().get("Location").unwrap(),
            "/?error=validation",
            "The API did not redirect to /?error=validation when the payload was {}.",
            description
        );
    }

    app.cleanup_test_db().await.unwrap();
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    // Arrange
    let app = spawn_app().await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let form_data = FormData {
        name: Some("abdo_test".to_string()),
        email: Some("abdo_test@gmail.com".to_string()),
        cf_turnstile_response: Some("test-token".to_string()),
    };
    // Act
    app.post_subscriptions(&form_data).await;
    // Assert
    // Mock asserts on drop
    // clean-up
    app.cleanup_test_db().await.unwrap();
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
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

    // Act
    app.post_subscriptions(&body).await;

    // Assert
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    // The two links should be identical
    assert_eq!(confirmation_links.html, confirmation_links.plain_text);

    app.cleanup_test_db().await.unwrap();
}
