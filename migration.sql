-- Add daily_goal to challenge and make group goal nullable
ALTER TABLE challenge ADD daily_goal real NULL;
ALTER TABLE challenge ALTER COLUMN goal DROP NOT NULL;

