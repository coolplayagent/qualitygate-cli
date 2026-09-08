# Merge request checks

`qualitygate check --mr <url>` accepts GitHub pull request URLs and GitLab merge request URLs, including nested GitLab groups. The local checkout's `origin` must identify the target repository. Git is required, and the policy configuration must be committed in the source snapshot.

```bash
qualitygate check --mr https://github.com/owner/repository/pull/123 --format json
qualitygate check --mr https://gitlab.example/group/project/-/merge_requests/123 --format markdown
```

The CLI obtains the source and target commit IDs from the provider, fetches missing objects through the existing Git `origin`, and computes their unique merge-base locally. Checks compare that base with the source commit, including when the target branch has advanced independently. GitLab's current target branch is read separately because the MR's cached diff refs describe a diff version. Provider schemas follow the [GitHub pull request API](https://docs.github.com/en/rest/pulls/pulls) and [GitLab merge request API](https://docs.gitlab.com/api/merge_requests/).

Fetching writes Git objects without changing branches, remote-tracking refs, tags, `FETCH_HEAD`, the index or worktree files. Commands execute against the disposable source snapshot. Shallow history, missing objects, ambiguous merge-bases, unsupported provider URLs and failed API calls produce incomplete validation (exit 2).

JSON evidence includes `snapshot.merge_request` with the provider, URL, repository, request number, branch names, source/target commit IDs, merge-base and hashes of the API response bodies. Table and Markdown show the comparison commits. Before completing a report, the CLI resolves the request again; source/target commit or branch changes invalidate the evidence, even when the merge-base and source contents stay unchanged. Diagnostic recheck commands retain the MR URL and API override.

## Authentication and endpoints

HTTPS is required. HTTP is allowed only for loopback provider fixtures, and credentials are never sent over HTTP. Responses are limited to 1 MiB; each HTTP request has a 5-second connection timeout and a 30-second total timeout. Redirects are not followed. Raw provider response bodies and authorization headers are not copied into reports.

- `GITHUB_TOKEN` is used only for the `api.github.com` API authority.
- `GITLAB_TOKEN` is used only for the `gitlab.com` API authority.
- Self-hosted services use `QUALITYGATE_MR_TOKEN` with `QUALITYGATE_MR_TOKEN_HOST` set to the exact API host, including its port when nondefault. Setting a token without a matching host does not send it.

Git object downloads use Git's existing authentication configuration independently of the HTTP token. The current HTTP client does not read system proxy configuration.

GitHub.com defaults to `https://api.github.com/`; other GitHub hosts use `/api/v3/`, and GitLab uses `/api/v4/`. `--mr-api-base <url>` overrides the API base on the MR host (or `api.github.com` for GitHub.com). It requires `--mr` and rejects embedded credentials, query strings and fragments. `--mr` is mutually exclusive with the other snapshot selectors and `--base`.

This command creates local reports. It does not post reviews or comments to the provider. Live service compatibility remains subject to platform verification; automated tests use bounded local HTTP fixtures and real temporary Git repositories, including an actual missing-object fetch.
