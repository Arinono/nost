-- Write your down sql migration here
drop trigger insert_follower_since;
drop trigger update_follower_since;
drop trigger insert_subscriber_since;
drop trigger update_subscriber_since;
drop trigger if exists insert_subgift;
drop trigger if exists insert_bit;
drop table latests;
