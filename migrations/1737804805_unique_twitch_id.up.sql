-- Write your up sql migration here
create unique index users_twitch_id_idx on users(twitch_id);
