-- Add migration script here
CREATE TABLE idempotency (
    -- uuid
    user_uuid TEXT NOT NULL REFERENCES users(uuid),
    idempotency_key TEXT NOT NULL,
    response_status_code INT NOT NULL,
    -- will use the json function in the backend
    -- json array of header's name: String and it's value: bytes
    response_headers TEXT NOT NULL,
    response_body BLOB NOT NULL,
    -- timestamptz
    created_at TEXT NOT NULL,
    PRIMARY KEY(user_uuid, idempotency_key)
);
