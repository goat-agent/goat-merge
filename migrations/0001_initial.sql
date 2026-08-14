create table app_credentials (
    only_row       integer primary key default 1 check (only_row = 1),
    app_id         bigint      not null,
    slug           text        not null,
    client_id      text        not null,
    private_key    bytea       not null,
    webhook_secret bytea       not null,
    client_secret  bytea       not null,
    created_at     timestamptz not null default now()
);

create table installations (
    id         bigint primary key,
    account    text        not null,
    removed_at timestamptz,
    created_at timestamptz not null default now()
);

create table repositories (
    id              bigint primary key,
    installation_id bigint      not null references installations (id) on delete cascade,
    owner           text        not null,
    name            text        not null,
    active          boolean     not null default false,
    created_at      timestamptz not null default now(),
    unique (owner, name)
);

create table queues (
    id             bigserial primary key,
    repository_id  bigint      not null references repositories (id) on delete cascade,
    branch         text        not null,
    paused         boolean     not null default false,
    paused_by      text,
    paused_at      timestamptz,
    base_sha       text,
    base_moved_at  timestamptz not null default now(),
    unique (repository_id, branch)
);

create table entries (
    id            bigserial primary key,
    queue_id      bigint      not null references queues (id) on delete cascade,
    pull_request  integer     not null,
    title         text        not null default '',
    requested_by  text        not null,
    requested_at  timestamptz not null default now(),
    queued_at     timestamptz,
    priority      integer     not null default 0,
    expedited_by  text,
    expedite_note text,
    status        text        not null,
    status_detail text        not null default '',
    settled_at    timestamptz,
    merged_sha    text,
    unique (queue_id, pull_request)
);

create index entries_running on entries (queue_id, priority desc, queued_at)
    where settled_at is null;

create table verifications (
    id                     bigserial primary key,
    entry_id               bigint      not null references entries (id) on delete cascade,
    base                   text        not null,
    head                   text        not null,
    candidate_branch       text        not null,
    candidate_pull_request integer,
    candidate_sha          text,
    conclusion             text        not null default 'pending',
    failed_checks          text[]      not null default '{}',
    started_at             timestamptz not null default now(),
    finished_at            timestamptz,
    discarded_at           timestamptz,
    discarded_because      text
);

create index verifications_live on verifications (entry_id, started_at desc)
    where discarded_at is null;

create table deliveries (
    id          text primary key,
    event       text        not null,
    payload     jsonb       not null,
    received_at timestamptz not null default now(),
    handled_at  timestamptz
);

create table jobs (
    id         bigserial primary key,
    kind       text        not null,
    subject    text        not null,
    run_after  timestamptz not null default now(),
    attempts   integer     not null default 0,
    last_error text,
    locked_at  timestamptz,
    locked_by  text,
    created_at timestamptz not null default now(),
    unique (kind, subject)
);

create index jobs_ready on jobs (run_after) where locked_at is null;

create table audit (
    id            bigserial primary key,
    at            timestamptz not null default now(),
    actor         text        not null,
    action        text        not null,
    repository_id bigint references repositories (id) on delete set null,
    pull_request  integer,
    detail        text        not null default ''
);

create index audit_by_repository on audit (repository_id, at desc);

create table sessions (
    token_digest bytea primary key,
    login        text        not null,
    access_token bytea       not null,
    created_at   timestamptz not null default now(),
    seen_at      timestamptz not null default now()
);
