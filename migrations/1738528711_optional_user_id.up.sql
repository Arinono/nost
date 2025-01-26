-- Write your up sql migration here

pragma foreign_keys=off;
create table new_subgifts (
  id integer primary key,
  user_id integer,
  number integer not null,
  tier text not null,
  created_at text not null,
  foreign key (user_id) references users (id) on delete cascade
);

insert into new_subgifts (
  id,
  user_id,
  number,
  tier,
  created_at
) select
  id,
  user_id,
  number,
  tier,
  created_at
from subgifts;

drop table subgifts;

alter table new_subgifts rename to subgifts;

create table new_bits (
  id integer primary key,
  user_id integer,
  number integer not null,
  message text,
  created_at text not null,
  foreign key (user_id) references users (id) on delete cascade
);

insert into new_bits (
  id,
  user_id,
  number,
  message,
  created_at
) select
  id,
  user_id,
  number,
  message,
  created_at
from bits;

drop table bits;

alter table new_bits rename to bits;

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
pragma foreign_keys=on;


