-- DROP TABLE IF EXISTS person;

CREATE TABLE IF NOT EXISTS person (
    id bigserial PRIMARY KEY ,
    name varchar(255) NOT NULL,
    email varchar(255) NOT NULL,
    phone varchar(255)
);
