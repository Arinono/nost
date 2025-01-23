-- Write your up sql migration here
create table if not exists users (
  id integer primary key,
  display_name text not null,
  twitch_id integer not null,
  follower_since text,
  subscriber_since text,
  subgift_total integer,
  subscription_tier text,
  created_at text not null,
  updated_at text not null,
  deleted_at text
);

create table if not exists subgifts (
  id integer primary key,
  user_id integer not null,
  number integer not null,
  tier text not null,
  created_at text not null,
  foreign key (user_id) references users (id) on delete cascade
);

create table if not exists bits (
  id integer primary key,
  user_id integer not null,
  number integer not null,
  message text,
  created_at text not null,
  foreign key (user_id) references users (id) on delete cascade
);
