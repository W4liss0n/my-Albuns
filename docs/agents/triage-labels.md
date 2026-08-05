# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the GitHub labels used by this repository.

| Canonical role      | GitHub label          | Meaning                                  |
| ------------------- | --------------------- | ---------------------------------------- |
| `needs-triage`      | `needs-triage`        | Maintainer must evaluate the issue       |
| `needs-info`        | `needs-info`          | Waiting for more information             |
| `ready-for-agent`   | `ready-for-agent`     | Fully specified and ready for an agent   |
| `ready-for-human`   | `ready-for-human`     | Requires human implementation or choice  |
| `wontfix`           | `wontfix`             | Will not be actioned                     |

Skills must use the label in the second column when recording a ticket's triage state.

## Wayfinder workflow states

Wayfinder child issues use `claimed` and `resolved` as semantic workflow states, not as triage labels. An open issue without an assignee is open and unclaimed; assignment to the driving developer claims it; closing it resolves it.
