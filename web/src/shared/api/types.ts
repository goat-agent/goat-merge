import type { Read } from "./reading";
import { list, maybe, need, oneOf, shape, text, whole, yesNo } from "./reading";

export type Status =
  | "Not queued"
  | "Waiting"
  | "Blocked"
  | "Queued"
  | "Preparing"
  | "Checking"
  | "Merging"
  | "Merged"
  | "Failed"
  | "Cancelled";

const statuses = [
  "Not queued",
  "Waiting",
  "Blocked",
  "Queued",
  "Preparing",
  "Checking",
  "Merging",
  "Merged",
  "Failed",
  "Cancelled",
] as const satisfies readonly Status[];

export type Trial = {
  base: string;
  head: string;
  candidate_branch: string;
  candidate_pull_request: number | null;
  candidate_sha: string | null;
  conclusion: string;
  failed_checks: string[];
  started_at: string;
};

export const readsAsATrial: Read<Trial> = shape((given) => ({
  base: need(text, given.base),
  head: need(text, given.head),
  candidate_branch: need(text, given.candidate_branch),
  candidate_pull_request: need(maybe(whole), given.candidate_pull_request),
  candidate_sha: need(maybe(text), given.candidate_sha),
  conclusion: need(text, given.conclusion),
  failed_checks: need(list(text), given.failed_checks),
  started_at: need(text, given.started_at),
}));

export type Row = {
  pull_request: number;
  title: string;
  status: Status;
  detail: string;
  requested_by: string;
  requested_at: string;
  queued_at: string | null;
  settled_at: string | null;
  expedited_by: string | null;
  expedite_note: string | null;
  merged_sha: string | null;
  attempt: Trial | null;
};

export const readsAsARow: Read<Row> = shape((given) => ({
  pull_request: need(whole, given.pull_request),
  title: need(text, given.title),
  status: need(oneOf(statuses), given.status),
  detail: need(text, given.detail),
  requested_by: need(text, given.requested_by),
  requested_at: need(text, given.requested_at),
  queued_at: need(maybe(text), given.queued_at),
  settled_at: need(maybe(text), given.settled_at),
  expedited_by: need(maybe(text), given.expedited_by),
  expedite_note: need(maybe(text), given.expedite_note),
  merged_sha: need(maybe(text), given.merged_sha),
  attempt: need(maybe(readsAsATrial), given.attempt),
}));

export type QueueView = {
  owner: string;
  name: string;
  branch: string;
  paused: boolean;
  paused_by: string | null;
  base_sha: string | null;
  entries: Row[];
};

export const readsAsAQueue: Read<QueueView> = shape((given) => ({
  owner: need(text, given.owner),
  name: need(text, given.name),
  branch: need(text, given.branch),
  paused: need(yesNo, given.paused),
  paused_by: need(maybe(text), given.paused_by),
  base_sha: need(maybe(text), given.base_sha),
  entries: need(list(readsAsARow), given.entries),
}));

export type RepositoryView = {
  owner: string;
  name: string;
  active: boolean;
  queues: { branch: string; paused: boolean; paused_by: string | null }[];
};

const readsAsABranch: Read<RepositoryView["queues"][number]> = shape((given) => ({
  branch: need(text, given.branch),
  paused: need(yesNo, given.paused),
  paused_by: need(maybe(text), given.paused_by),
}));

export const readsAsARepository: Read<RepositoryView> = shape((given) => ({
  owner: need(text, given.owner),
  name: need(text, given.name),
  active: need(yesNo, given.active),
  queues: need(list(readsAsABranch), given.queues),
}));

export type Insights = {
  merged: number;
  failed: number;
  cancelled: number;
  median_wait_seconds: number | null;
  p95_wait_seconds: number | null;
  reasons: { reason: string; count: number }[];
  merged_by_day: { day: string; count: number }[];
};

const readsAsAReason: Read<Insights["reasons"][number]> = shape((given) => ({
  reason: need(text, given.reason),
  count: need(whole, given.count),
}));

const readsAsADay: Read<Insights["merged_by_day"][number]> = shape((given) => ({
  day: need(text, given.day),
  count: need(whole, given.count),
}));

export const readsAsInsights: Read<Insights> = shape((given) => ({
  merged: need(whole, given.merged),
  failed: need(whole, given.failed),
  cancelled: need(whole, given.cancelled),
  median_wait_seconds: need(maybe(whole), given.median_wait_seconds),
  p95_wait_seconds: need(maybe(whole), given.p95_wait_seconds),
  reasons: need(list(readsAsAReason), given.reasons),
  merged_by_day: need(list(readsAsADay), given.merged_by_day),
}));

export type Advice = { level: "info" | "warning" | "error"; text: string };

const readsAsAdvice: Read<Advice> = shape((given) => ({
  level: need(oneOf(["info", "warning", "error"] as const), given.level),
  text: need(text, given.text),
}));

export type Diagnosis = {
  owner: string;
  name: string;
  branch: string;
  active: boolean;
  protection: { declared: boolean; required_checks: string[]; required_approvals: number };
  merge_methods: string[];
  label: { name: string; exists: boolean };
  enforced: boolean;
  check_name: string;
  config: string | null;
  fork_workflow: boolean;
  advice: Advice[];
};

const readsAsProtection: Read<Diagnosis["protection"]> = shape((given) => ({
  declared: need(yesNo, given.declared),
  required_checks: need(list(text), given.required_checks),
  required_approvals: need(whole, given.required_approvals),
}));

const readsAsALabel: Read<Diagnosis["label"]> = shape((given) => ({
  name: need(text, given.name),
  exists: need(yesNo, given.exists),
}));

export const readsAsADiagnosis: Read<Diagnosis> = shape((given) => ({
  owner: need(text, given.owner),
  name: need(text, given.name),
  branch: need(text, given.branch),
  active: need(yesNo, given.active),
  protection: need(readsAsProtection, given.protection),
  merge_methods: need(list(text), given.merge_methods),
  label: need(readsAsALabel, given.label),
  enforced: need(yesNo, given.enforced),
  check_name: need(text, given.check_name),
  config: need(maybe(text), given.config),
  fork_workflow: need(yesNo, given.fork_workflow),
  advice: need(list(readsAsAdvice), given.advice),
}));

export type Note = { at: string; actor: string; action: string; detail: string };

const readsAsANote: Read<Note> = shape((given) => ({
  at: need(text, given.at),
  actor: need(text, given.actor),
  action: need(text, given.action),
  detail: need(text, given.detail),
}));

export type Timeline = { entry: Row; attempts: Trial[]; timeline: Note[] };

export const readsAsATimeline: Read<Timeline> = shape((given) => ({
  entry: need(readsAsARow, given.entry),
  attempts: need(list(readsAsATrial), given.attempts),
  timeline: need(list(readsAsANote), given.timeline),
}));

export type Health = { ok: boolean; set_up: boolean; version: string };

export const readsAsHealth: Read<Health> = shape((given) => ({
  ok: need(yesNo, given.ok),
  set_up: need(yesNo, given.set_up),
  version: need(text, given.version),
}));

export type Viewer = { login: string };

export const readsAsAViewer: Read<Viewer> = shape((given) => ({
  login: need(text, given.login),
}));

export type CliToken = { token: string; url: string };

export const readsAsACliToken: Read<CliToken> = shape((given) => ({
  token: need(text, given.token),
  url: need(text, given.url),
}));

export type Manifest = {
  personal: string;
  organization: string;
  manifest: string;
  already_set_up: boolean;
};

export const readsAsAManifest: Read<Manifest> = shape((given) => ({
  personal: need(text, given.personal),
  organization: need(text, given.organization),
  manifest: need(text, given.manifest),
  already_set_up: need(yesNo, given.already_set_up),
}));

export type Enabled = { ok: boolean; branch: string; config_pull_request: number | null };

export const readsAsEnabled: Read<Enabled> = shape((given) => ({
  ok: need(yesNo, given.ok),
  branch: need(text, given.branch),
  config_pull_request: need(maybe(whole), given.config_pull_request),
}));
