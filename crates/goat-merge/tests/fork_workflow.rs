use goat_merge::github::look::declares_no_secrets;

fn the_example_we_tell_people_to_copy() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/merge-queue-fork.example.yml"
    );
    std::fs::read_to_string(path)
        .expect("the example workflow the README points at should be there")
}

#[test]
fn the_example_workflow_we_tell_people_to_copy_is_one_we_would_accept() {
    let example = the_example_we_tell_people_to_copy();

    assert!(
        declares_no_secrets(&example),
        "the README tells people to copy this file and says fork pull requests are blocked \
         until they do; if we then refuse the very file we handed them, we have told them to \
         do something and punished them for doing it"
    );
}

#[test]
fn a_workflow_is_judged_by_what_it_runs_not_by_what_its_comments_talk_about() {
    let talks_about_the_rules = "\
permissions:
  contents: read
jobs:
  verify:
    steps:
      - run: make test

# Keep it that way:
#   - no `secrets.` anywhere, and no `secrets:` passed to a reusable workflow
#   - every permission read, never write, never write-all
";

    assert!(
        declares_no_secrets(talks_about_the_rules),
        "a comment explaining the rules is not the workflow breaking them"
    );
}

#[test]
fn a_workflow_that_reaches_for_a_secret_is_refused() {
    let helps_itself = "\
permissions:
  contents: read
jobs:
  verify:
    steps:
      - run: deploy --token ${{ secrets.GITHUB_TOKEN }}
";

    assert!(!declares_no_secrets(helps_itself));
}

#[test]
fn a_workflow_that_passes_secrets_on_to_another_one_is_refused() {
    let hands_them_over = "\
permissions:
  contents: read
jobs:
  verify:
    uses: ./.github/workflows/inner.yml
    secrets: inherit
";

    assert!(!declares_no_secrets(hands_them_over));
}

#[test]
fn a_workflow_that_asks_for_a_write_permission_is_refused() {
    let wants_to_write = "\
permissions:
  contents: write
jobs:
  verify:
    steps:
      - run: make test
";
    let wants_everything = "\
permissions: write-all
jobs:
  verify:
    steps:
      - run: make test
";

    assert!(!declares_no_secrets(wants_to_write));
    assert!(!declares_no_secrets(wants_everything));
}

#[test]
fn a_workflow_that_never_says_what_it_may_do_is_refused() {
    let says_nothing = "\
jobs:
  verify:
    steps:
      - run: make test
";

    assert!(
        !declares_no_secrets(says_nothing),
        "without a permissions block the workflow gets whatever the repository hands out by \
         default, which is the thing this check exists to stop"
    );
}
