alter table queues add column verify_at_once integer not null default 1;

alter table verifications add column queue_id bigint references queues (id) on delete cascade;

create table verification_members (
    verification_id bigint  not null references verifications (id) on delete cascade,
    entry_id        bigint  not null references entries (id) on delete cascade,
    head            text    not null,
    place           integer not null,
    primary key (verification_id, entry_id)
);

create index verification_members_by_entry on verification_members (entry_id);

insert into verification_members (verification_id, entry_id, head, place)
select id, entry_id, head, 0 from verifications;

update verifications v set queue_id = e.queue_id from entries e where e.id = v.entry_id;

alter table verifications alter column queue_id set not null;

drop index verifications_live;

alter table verifications drop column entry_id;

alter table verifications drop column head;

create index verifications_live on verifications (queue_id, started_at desc)
    where discarded_at is null;
