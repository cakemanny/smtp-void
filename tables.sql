
create table mail (
    mail_id bigint not null auto_increment,
    `from` varchar(255) not null,
    compressed_data blob not null,
    received timestamp not null default current_timestamp,

    primary key (mail_id)
) engine=InnoDb default charset=utf8;

create table rcpt (
    rcpt_id bigint not null auto_increment,
    mail_id bigint not null,
    rcpt varchar(255) not null,

    primary key (rcpt_id),
    foreign key (mail_id) references mail (mail_id)
) engine=InnoDb default charset=utf8;

create view mail_view as
select mail_id
     , `from`
     , `to`
     , uncompress(compressed_data) as data
  from mail
  join (  select mail_id, group_concat(rcpt) as `to`
            from rcpt
        group by mail_id) rcpts using (mail_id)
;

-- second idea for mail_view
-- might work better when wanting to look at single rows
select mail_id
     , `from`
     , (select group_concat(rcpt)
          from rcpt
         where mail_id = mail.mail_id) as `to`
     , uncompress(compressed_data) as data
     , received
  from mail
;




