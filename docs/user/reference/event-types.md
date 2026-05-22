# Event Types and Session IDs

This reference lists the GitHub webhook event types that Queue-Keeper recognises and the session ID pattern each produces. Session IDs are used for ordered delivery and appear in the `session_id` field of every `WrappedEvent`.

Session ID format: `{owner}/{repo}/{entity_type}/{entity_id}`

---

## Events with session IDs by entity type

### Pull Requests — `pull_request/{pr_number}`

Applies to all actions: `opened`, `closed`, `reopened`, `synchronize`, `labeled`, `unlabeled`, `assigned`, `unassigned`, `review_requested`, `review_request_removed`, `edited`, `ready_for_review`, `converted_to_draft`, `locked`, `unlocked`.

```
myorg/myrepo/pull_request/42
```

Also applies to `pull_request_review`, `pull_request_review_comment`, and `pull_request_review_thread` events — all share the same PR-scoped session ID.

### Issues — `issue/{issue_number}`

Applies to all actions: `opened`, `closed`, `reopened`, `labeled`, `unlabeled`, `assigned`, `unassigned`, `edited`, `deleted`, `pinned`, `unpinned`, `locked`, `unlocked`, `milestoned`, `demilestoned`.

```
myorg/myrepo/issue/17
```

Also applies to `issue_comment` events.

### Pushes — `branch/{branch_name}` or `branch/--tags`

```
myorg/myrepo/branch/main
myorg/myrepo/branch/feature%2Fmy-branch
myorg/myrepo/branch/--tags            # for tag push events
```

Branch names containing `/` are percent-encoded.

### Releases — `release/{tag_name}`

Applies to all actions: `published`, `unpublished`, `created`, `edited`, `deleted`, `prereleased`, `released`.

```
myorg/myrepo/release/v1.2.0
```

### Workflow Runs — `workflow_run/{run_id}`

Applies to `workflow_run` and `workflow_job` events.

```
myorg/myrepo/workflow_run/9999
```

### Discussions — `discussion/{discussion_number}`

Applies to `discussion` and `discussion_comment` events.

```
myorg/myrepo/discussion/42
```

### Teams — `team/{team_slug}`

Applies to `team` and `team_add` events.

```
myorg/myrepo/team/backend-team
```

### Repository-level events — `repository/repository`

Applies to events that operate on the repository itself rather than an entity within it: `repository`, `push` to default branch (repository level), `star`, `watch`, `fork`, `delete`, `public`, `repository_import`, `repository_vulnerability_alert`, `security_advisory`.

```
myorg/myrepo/repository/repository
```

### Deployments — `deployment/{deployment_id}`

Applies to `deployment` and `deployment_status` events.

```
myorg/myrepo/deployment/12345
```

### Check Runs and Suites — `check_run/{check_run_id}` / `check_suite/{check_suite_id}`

```
myorg/myrepo/check_run/98765
myorg/myrepo/check_suite/11111
```

### Unknown or unrecognised events — `unknown/unknown`

When Queue-Keeper receives a GitHub event type it does not recognise, it falls back to:

```
myorg/myrepo/unknown/unknown
```

---

## Event type subscription patterns

Use these patterns in `bot-config.yaml`'s `events` field:

| Pattern | Matches |
|---|---|
| `pull_request.opened` | Only PR opened events |
| `pull_request.*` | All pull request actions |
| `issues.*` | All issue actions |
| `push` | All push events (no action sub-type) |
| `release.published` | Only published releases |
| `workflow_run.completed` | Only completed workflow runs |
| `*` | Every event from every provider |
| `issues.*`, `!issues.deleted` | All issue events except deleted |

---

## Summary table

| GitHub event | Entity type | Session ID example |
|---|---|---|
| `pull_request.*` | `pull_request` | `myorg/myrepo/pull_request/42` |
| `pull_request_review` | `pull_request` | `myorg/myrepo/pull_request/42` |
| `pull_request_review_comment` | `pull_request` | `myorg/myrepo/pull_request/42` |
| `issues.*` | `issue` | `myorg/myrepo/issue/17` |
| `issue_comment` | `issue` | `myorg/myrepo/issue/17` |
| `push` (branch) | `branch` | `myorg/myrepo/branch/main` |
| `release.*` | `release` | `myorg/myrepo/release/v1.2.0` |
| `workflow_run` | `workflow_run` | `myorg/myrepo/workflow_run/9999` |
| `workflow_job` | `workflow_run` | `myorg/myrepo/workflow_run/9999` |
| `discussion.*` | `discussion` | `myorg/myrepo/discussion/42` |
| `discussion_comment` | `discussion` | `myorg/myrepo/discussion/42` |
| `team`, `team_add` | `team` | `myorg/myrepo/team/backend` |
| `deployment` | `deployment` | `myorg/myrepo/deployment/12345` |
| `deployment_status` | `deployment` | `myorg/myrepo/deployment/12345` |
| `check_run` | `check_run` | `myorg/myrepo/check_run/98765` |
| `check_suite` | `check_suite` | `myorg/myrepo/check_suite/11111` |
| `repository`, `star`, `fork`, … | `repository` | `myorg/myrepo/repository/repository` |
| (unrecognised) | `unknown` | `myorg/myrepo/unknown/unknown` |
