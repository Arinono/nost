-- Write your up sql migration here
create table latests (
  id integer primary key,
  follower integer default null,
  subscriber integer default null,
  subgift integer default null,
  bit integer default null,
  foreign key (follower) references users(id),
  foreign key (subscriber) references users(id),
  foreign key (subgift) references subgifts(id),
  foreign key (bit) references bits(id)
);

create trigger if not exists insert_follower_since
  after insert on users
  begin
    insert into latests (id, follower)
      values (1, (
        select id from users
          where deleted_at is null
          and follower_since is not null
        order by follower_since desc
        limit 1
      )) on conflict (id)
      do update set follower = excluded.follower;
end;

create trigger if not exists update_follower_since
  after update on users
  begin
    insert into latests (id, follower)
      values (1, (
        select id from users
          where deleted_at is null
          and follower_since is not null
        order by follower_since desc
        limit 1
      )) on conflict (id)
      do update set follower = excluded.follower;
end;

create trigger if not exists insert_subscriber_since
  after insert on users
  begin
    insert into latests (id, subscriber)
      values (1, (
        select id from users
          where deleted_at is null
          and subscriber_since is not null
        order by subscriber_since desc
        limit 1
      )) on conflict (id)
      do update set subscriber = excluded.subscriber;
end;

create trigger if not exists update_subscriber_since
  after update on users
  begin
    insert into latests (id, subscriber)
      values (1, (
        select id from users
          where deleted_at is null
          and subscriber_since is not null
        order by subscriber_since desc
        limit 1
      )) on conflict (id)
      do update set subscriber = excluded.subscriber;
end;

create trigger if not exists insert_subgift
  after insert on subgifts
  begin
    insert into latests (id, subgift)
      values (1, (
        select s.id from subgifts s
          inner join users u on u.id = s.user_id
          where u.deleted_at is null
        order by s.created_at desc
        limit 1
      )) on conflict (id)
      do update set subgift = excluded.subgift;

    update users
      set subgift_total = (select sum(number) from subgifts where user_id = new.user_id)
    where id = new.user_id;
end;

create trigger if not exists insert_bit
  after insert on bits
  begin
    insert into latests (id, bit)
      values (1, (
        select b.id from bits b
          inner join users u on u.id = b.user_id
          where u.deleted_at is null
        order by b.created_at desc
        limit 1
      )) on conflict (id)
      do update set bit = excluded.bit;
end;
