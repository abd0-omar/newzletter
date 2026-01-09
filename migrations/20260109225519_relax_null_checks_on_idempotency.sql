-- Add migration script here
-- ALTER TABLE idempotency ALTER COLUMN response_status_code DROP NOT NULL;
-- ALTER TABLE idempotency ALTER COLUMN response_body DROP NOT NULL;
-- ALTER TABLE idempotency ALTER COLUMN response_headers DROP NOT NULL;

-- relax null constraints on response
CREATE TABLE idempotency_new (
    -- uuid
    user_uuid TEXT NOT NULL REFERENCES users(uuid),
    idempotency_key TEXT NOT NULL,
    response_status_code INT,
    -- will use the json function in the backend
    -- json array of header's name: String and it's value: bytes
    response_headers TEXT,
    response_body BLOB,
    -- timestamptz
    created_at TEXT NOT NULL,
    PRIMARY KEY(user_uuid, idempotency_key)
);

INSERT INTO idempotency_new SELECT * FROM idempotency;

DROP TABLE idempotency;

ALTER TABLE idempotency_new RENAME TO idempotency;
