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
CREATE TABLE public.session_type (
	id serial4 NOT NULL,
	name varchar(255) NOT NULL,
	requires_trainer bool DEFAULT true NULL,
	cost int2 DEFAULT 0 NULL,
	CONSTRAINT session_type_cost_check CHECK ((cost >= 0)),
	CONSTRAINT session_type_name_key UNIQUE (name),
	CONSTRAINT session_type_pkey PRIMARY KEY (id)
);
INSERT INTO session_type
    (name, cost)
VALUES
    ('HIIT', 1),
    ('Strong', 1),
    ('On The Move', 1)
ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS session(
	id bigserial PRIMARY KEY,
	datetime timestamptz NOT NULL,
	duration_mins int4 NOT NULL,
	session_type int4 NOT NULL REFERENCES session_type,
	location int4 NULL REFERENCES location,
	trainer int8 NULL REFERENCES person,
	max_booking_count int8 NULL,
	notes text NULL,
	cost int2 DEFAULT 0 NOT NULL CHECK ((cost >= 0))
);

CREATE TABLE IF NOT EXISTS booking (
    person_id bigint NOT NULL REFERENCES person ON DELETE CASCADE,
    session_id bigint NOT NULL REFERENCES session ON DELETE CASCADE,
    attended bool DEFAULT false NOT NULL,
	credits_used int2 DEFAULT 0 NULL CHECK ((credits_used >= 0)),
    PRIMARY KEY (person_id, session_id)
);
