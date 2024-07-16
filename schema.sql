-- DROP TABLE IF EXISTS person;

-- user tables
CREATE TABLE IF NOT EXISTS person (
    id bigserial PRIMARY KEY,
    name text NOT NULL,
    email text NOT NULL UNIQUE,
    phone text,
    pwd text,
    roles text
);
CREATE TABLE IF NOT EXISTS temp_password (
    person_id bigint UNIQUE NOT NULL REFERENCES person ON DELETE CASCADE,
    pwd text NOT NULL,
    sent timestamp with time zone NOT NULL,
    expiry timestamp with time zone NOT NULL
);

-- location table and data
CREATE TABLE IF NOT EXISTS location (
    id serial PRIMARY KEY,
    name varchar(255) UNIQUE NOT NULL,
    address varchar(1023)
);
INSERT INTO location
    (name, address)
VALUES
    ('Oak Hill Park', 'Oak Hill Park, Parkside Gardens, London EN4 8JP'),
    ('Trent Park', 'Trent Park, London EN4 0PS')
ON CONFLICT DO NOTHING;

-- session tables
CREATE TABLE IF NOT EXISTS session_type (
    id serial PRIMARY KEY,
    name varchar(255) UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS session (
    id bigserial PRIMARY KEY,
    datetime timestamp with time zone NOT NULL,
    duration_mins integer NOT NULL,
    session_type integer NOT NULL REFERENCES session_type,
    location integer NOT NULL REFERENCES location,
    trainer bigint NOT NULL REFERENCES person,
    max_booking_count BIGINT
);

CREATE TABLE IF NOT EXISTS booking (
    person_id bigint NOT NULL REFERENCES person ON DELETE CASCADE,
    session_id bigint NOT NULL REFERENCES session ON DELETE CASCADE,
    PRIMARY KEY (person_id, session_id)
);
