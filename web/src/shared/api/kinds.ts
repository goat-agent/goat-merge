const kinds = {
  not_signed_in: null,
  not_set_up: null,
  not_installed_here: null,
  not_allowed: null,
  no_queue_here: null,
  never_queued: null,
  queue_is_off: null,
  github_refused: null,
  github_is_rate_limiting: null,
  github_is_unreachable: null,
  configuration_is_invalid: null,
  reason_required: null,
  malformed_request: null,
  no_such_endpoint: null,
  broken: null,
  cannot_reach_the_server: null,
  answer_was_not_json: null,
} satisfies Record<string, null>;

export type Kind = keyof typeof kinds;

export function isKind(value: unknown): value is Kind {
  return typeof value === "string" && Object.hasOwn(kinds, value);
}
