alter table session_type add column cost smallint default 0 check (cost >= 0);
alter table session add column cost smallint default 0 check (cost >= 0);