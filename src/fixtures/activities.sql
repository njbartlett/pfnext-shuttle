CREATE OR REPLACE FUNCTION insert_activity(
    date DATE,
    amount REAL,
    person_name TEXT,
    challenge_name TEXT
) RETURNS BIGINT AS $$
DECLARE
    new_activity_id BIGINT;
BEGIN
    INSERT INTO activity (person_id, challenge_id, date, amount)
    SELECT p.id, c.id, date, amount
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