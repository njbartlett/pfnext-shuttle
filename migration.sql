-- Per-member read tracking for blog posts
CREATE TABLE IF NOT EXISTS post_read (
    person_id bigint NOT NULL REFERENCES person ON DELETE CASCADE,
    post_id bigint NOT NULL REFERENCES post ON DELETE CASCADE,
    read_at timestamptz NOT NULL,
    PRIMARY KEY (person_id, post_id)
);
CREATE INDEX IF NOT EXISTS post_read_post_id_idx ON post_read (post_id);

-- When each person record was created. Records that predate the column are
-- given midnight BST on 1 April 2024; new records default to the current time.
-- (Adding the column with a constant default fills the existing rows; changing
-- the default afterwards affects only rows inserted later.)
BEGIN;
ALTER TABLE person ADD COLUMN IF NOT EXISTS created_at timestamptz NOT NULL DEFAULT '2024-04-01 00:00:00+01';
ALTER TABLE person ALTER COLUMN created_at SET DEFAULT now();
COMMIT;
