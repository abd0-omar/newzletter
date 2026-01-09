pub mod authentication;
pub mod configuration;
pub mod domain;
pub mod email_client;
pub mod idempotency;
pub mod issue_delivery_worker;
pub mod routes;
pub mod session_state;
pub mod startup;
pub mod telemetry;
pub mod utils;

/*
// https://github.com/tokio-rs/axum/blob/main/examples/customize-extractor-error/src/with_rejection.rs
// to return a custom 400 error instead of 422 when form data is not correct
// #[derive(Debug, thiserror::Error)]
// #[error(transparent)]
// pub struct CustomFormRejectionReturn400Instead422(#[from] FormRejection);

// impl IntoResponse for CustomFormRejectionReturn400Instead422 {
//     fn into_response(self) -> axum::response::Response {
//         match self.0 {
//             FormRejection::FailedToDeserializeFormBody(_failed_to_deserialize_form_body) => {
//                 StatusCode::BAD_REQUEST.into_response()
//             }
//             _ => self.0.into_response(),
//         }
//     }
// }

// pub async fn subscribe(
//     WithRejection(Form(form_data), _): WithRejection<
//         Form<FormData>,
//         CustomFormRejectionReturn400Instead422,
//     >,
*/
