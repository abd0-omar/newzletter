-- Allow publishing a newsletter issue to multiple confirmed subscribers
-- and allow different issues to reuse the same title.
--
-- We first copy and drop the delivery queue table to remove the foreign-key
-- dependency on `newsletter_issues`, then rebuild both tables with the
-- intended constraints and restore the queued rows.

CREATE TABLE issue_delivery_queue_backup (
    newsletter_issue_uuid TEXT NOT NULL,
    subscriber_email TEXT NOT NULL
);

INSERT INTO issue_delivery_queue_backup (
    newsletter_issue_uuid,
    subscriber_email
)
SELECT
    newsletter_issue_uuid,
    subscriber_email
FROM issue_delivery_queue;

DROP TABLE issue_delivery_queue;

CREATE TABLE newsletter_issues_new (
    id INTEGER PRIMARY KEY,
    newsletter_issue_uuid TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    text_content TEXT NOT NULL,
    html_content TEXT NOT NULL,
    published_at TEXT NOT NULL
);

INSERT INTO newsletter_issues_new (
    id,
    newsletter_issue_uuid,
    title,
    text_content,
    html_content,
    published_at
)
SELECT
    id,
    newsletter_issue_uuid,
    title,
    text_content,
    html_content,
    published_at
FROM newsletter_issues;

DROP TABLE newsletter_issues;
ALTER TABLE newsletter_issues_new RENAME TO newsletter_issues;

CREATE TABLE issue_delivery_queue (
    id INTEGER PRIMARY KEY,
    newsletter_issue_uuid TEXT NOT NULL
        REFERENCES newsletter_issues(newsletter_issue_uuid),
    subscriber_email TEXT NOT NULL,
    UNIQUE(newsletter_issue_uuid, subscriber_email)
);

INSERT INTO issue_delivery_queue (
    newsletter_issue_uuid,
    subscriber_email
)
SELECT
    newsletter_issue_uuid,
    subscriber_email
FROM issue_delivery_queue_backup;

DROP TABLE issue_delivery_queue_backup;
