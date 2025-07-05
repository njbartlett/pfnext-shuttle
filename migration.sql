ALTER TABLE activity
ADD COLUMN activity_type int4 NULL REFERENCES activity_type;

ALTER TABLE activity ALTER COLUMN challenge_id DROP NOT NULL;

UPDATE activity 
SET activity_type = (
	SELECT challenge.activity_type
	FROM challenge
	WHERE challenge.id = activity.challenge_id
);

