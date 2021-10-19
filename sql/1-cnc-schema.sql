-- Table Structure
-- Primary Key
-- Creation Time
-- Creator User Id (if applicable)
-- Everything else

CREATE DATABASE cnc;
\c cnc;

drop table 

drop table if exists auth_card_t cascade;
create table auth_card_t(
  auth_card_id text not null,
  creation_time bigint not null,
  creator_user_id bigint not null,
  school_id bigint not null
);

drop table if exists auth_card_data_t cascade;
create table auth_card_data_t(
  auth_card_data_id bigserial not null,
  creation_time bigint not null,
  creator_user_id bigint not null,
  auth_card_id text not null,
  description text not null,
  active bool not null
);

create view recent_auth_card_data_v as
  select acd.* from auth_card_data_t acd
  inner join (
   select max(auth_card_data_id) id 
   from auth_card_data_t 
   group by auth_card_id
  ) maxids
  on maxids.id = acd.auth_card_data_id;

-- Invariant data about a scanner
drop table if exists scanner_t cascade;
create table scanner_t (
  scanner_id text not null primary key,
  creation_time bigint not null,
  creator_user_id bigint not null,
  auth_card_id text not null references auth_card_t(auth_card_id)
);

-- Mutable data about a scanner 
drop table if exists scanner_data_t cascade;
create table scanner_data_t(
  scanner_data_id bigserial not null primary key,
  creation_time bigint not null,
  creator_user_id bigint not null,
  scanner_id text not null references scanner_t(scanner_id),
  location_id bigint not null,
  description text not null,
  active bool not null
);

create view recent_scanner_data_v as
  select sd.* from scanner_data_t sd
  inner join (
   select max(scanner_data_id) id 
   from scanner_data_t 
   group by scanner_id
  ) maxids
  on maxids.id = sd.scanner_data_id;

-- This cache is calculated every so often, and is used to reduce the cost of expensive queries
-- place fields for expensive operations that need to access data from the 
drop table if exists scanner_cache_t;
create table scanner_cache_t(
  scanner_cache_id bigserial not null primary key,
  creation_time bigint not null,
  creator_user_id bigint not null,
  scanner_id text not null references scanner_t(scanner_id),
  average_ping_ms bigint not null, -- the average time it takes to ping it
  lifetime_uses_count bigint not null, -- how many times this scanner has ever been used
  month_uses_count bigint not null -- how many times this scanner has been used in the past 30 days
);

-- This is a command from the server to a scanner
drop table if exists command_t cascade;
create table command_t(
  command_id bigserial not null,
  creation_time bigint not null,
  creator_user_id bigint not null,
  scanner_id text not null references scanner_t(scanner_id),
  command_kind bigint not null, -- POWER_CYCLE | FULL_RESET | FLASH | BEEP
); 

-- This is the response of a scanner.
drop table id exists command_ack_t cascade;
create table command_t_ack(
  command_ack_id bigserial not null primary key,
  creation_time bigint not null,
  command_id bigint not null references command_t(command_id)
);
