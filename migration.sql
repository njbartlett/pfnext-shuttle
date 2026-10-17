-- Per-member read tracking for blog posts
CREATE TABLE IF NOT EXISTS post_read (
    person_id bigint NOT NULL REFERENCES person ON DELETE CASCADE,
    post_id bigint NOT NULL REFERENCES post ON DELETE CASCADE,
    read_at timestamptz NOT NULL,
    PRIMARY KEY (person_id, post_id)
);
CREATE INDEX IF NOT EXISTS post_read_post_id_idx ON post_read (post_id);
