INSERT INTO challenge (name, start, finish, goal, individual_goal, activity_type) VALUES
    ('April 2025 Hikes', '2025-04-01', '2025-04-30', 5000, 500, (SELECT id FROM activity_type WHERE name = 'Hiking')),
    ('May 2025 Cycling', '2025-05-01', '2025-05-31', 10000, 1000, (SELECT id FROM activity_type WHERE name = 'Cycling')),
    ('June Million Steps', '2025-06-01', '2025-05-30', 1000000, 300000, (SELECT id FROM activity_type WHERE name = 'Steps'));
