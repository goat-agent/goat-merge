alter table verifications add column built_on bigint references verifications (id) on delete set null;
alter table verifications add column depth integer not null default 0;

create table verification_assumptions (
    verification_id bigint  not null references verifications (id) on delete cascade,
    entry_id        bigint  not null references entries (id) on delete cascade,
    head            text    not null,
    place           integer not null,
    primary key (verification_id, entry_id)
);

create index verification_assumptions_by_entry on verification_assumptions (entry_id);

alter table entries add column merged_head text;

alter table queues add column speculate_to integer not null default 1;
alter table queues add column speculate_to_because text;
