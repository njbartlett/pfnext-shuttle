-- DROP TABLE IF EXISTS person;

CREATE TABLE IF NOT EXISTS person (
    id bigserial PRIMARY KEY,
    name varchar(255) NOT NULL,
    email varchar(255) NOT NULL,
    phone varchar(255)
);

CREATE TABLE IF NOT EXISTS session_type (
    id serial PRIMARY KEY,
    name varchar(255) UNIQUE NOT NULL
);
INSERT INTO session_type (name) VALUES ('Outdoor') ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS location (
    id serial PRIMARY KEY,
    name varchar(255) UNIQUE NOT NULL,
    address varchar(1023)
);
INSERT INTO location (name, address) VALUES('Oak Hill Park', 'Oak Hill Park, Parkside Gardens, London EN4 8JP') ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS session (
    id bigserial PRIMARY KEY,
    datetime timestamp with time zone NOT NULL,
    duration_mins integer NOT NULL,
    session_type integer NOT NULL REFERENCES session_type,
    location integer NOT NULL REFERENCES location
);