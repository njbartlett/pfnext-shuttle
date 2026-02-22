CREATE OR REPLACE FUNCTION insert_activity(
    date DATE,
    amount REAL,
    person_name TEXT,
    challenge_name TEXT
) RETURNS BIGINT AS $$
DECLARE
    new_activity_id BIGINT;
BEGIN
    INSERT INTO activity (person_id, challenge_id, activity_type, date, amount)
    SELECT p.id, c.id, c.activity_type, date, amount
    FROM person AS p CROSS JOIN challenge AS c
    WHERE p.name = person_name
    AND c.name = challenge_name
    RETURNING id INTO new_activity_id;

    RETURN new_activity_id;
END;
$$ LANGUAGE plpgsql;

SELECT insert_activity('2025-04-01', 100, 'user1', 'April 2025 Hikes');
SELECT insert_activity('2025-04-02', 200, 'user1', 'April 2025 Hikes');
SELECT insert_activity('2025-04-02', 400, 'user2', 'April 2025 Hikes');

SELECT insert_activity('2025-05-01', 100, 'user1', 'May 2025 Cycling');

-- Activities for a challenge that has a daily goal
SELECT insert_activity('2025-04-1', 5, 'user1', 'Marching Along');
SELECT insert_activity('2025-04-2', 6, 'user1', 'Marching Along');
SELECT insert_activity('2025-04-3', 4, 'user1', 'Marching Along'); -- Activity that does NOT reach the daily goal of 5.
SELECT insert_activity('2025-04-4', 3, 'user1', 'Marching Along'); -- Activity below the daily goal...
SELECT insert_activity('2025-04-4', 3, 'user1', 'Marching Along'); -- ... but summing with this activity to exceed the daily goal.

SELECT insert_activity('2025-04-01', 5, 'user2', 'Marching Along');