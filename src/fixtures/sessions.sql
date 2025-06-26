INSERT INTO session
    (datetime, duration_mins, session_type, location, trainer, max_booking_count, notes, cost)
SELECT '2025-01-01T09:00:00Z', 60, t.id, l.id, p.id, 0, '', 1
FROM session_type AS t CROSS JOIN location AS l CROSS JOIN person AS p
WHERE t.name = 'HIIT' AND l.name = 'Oak Hill Park' AND p.email = 'user1@example.com'