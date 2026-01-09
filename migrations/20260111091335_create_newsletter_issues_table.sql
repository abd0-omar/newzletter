-- Add migration script here
CREATE TABLE newsletter_issues (
    id INTEGER,
    newsletter_issue_uuid TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL UNIQUE,
    text_content TEXT NOT NULL,
    html_content TEXT NOT NULL,
    published_at TEXT NOT NULL,
    PRIMARY KEY(id)
);
