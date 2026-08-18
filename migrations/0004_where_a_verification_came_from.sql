alter table verifications
    add column narrowed_from bigint references verifications (id) on delete set null;

alter table queues add column verify_at_once_because text;
