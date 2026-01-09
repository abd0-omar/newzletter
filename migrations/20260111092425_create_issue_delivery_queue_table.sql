-- Add migration script here
CREATE TABLE issue_delivery_queue (
    id INTEGER,
    newsletter_issue_uuid TEXT NOT NULL UNIQUE
        REFERENCES newsletter_issues(newsletter_issue_uuid),
    subscriber_email TEXT NOT NULL,
    PRIMARY KEY(id, subscriber_email)
);
