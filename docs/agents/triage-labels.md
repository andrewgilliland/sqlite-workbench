# Triage labels

Map canonical triage roles to the actual GitHub label strings below.

| Canonical role | Tracker label | Meaning |
| --- | --- | --- |
| `needs-triage` | `needs-triage` | Maintainer needs to evaluate the issue |
| `needs-info` | `needs-info` | Waiting on reporter information |
| `ready-for-agent` | `ready-for-agent` | Fully specified; ready for autonomous implementation |
| `ready-for-human` | `ready-for-human` | Requires human implementation |
| `wontfix` | `wontfix` | Will not be actioned |

When a skill mentions a triage role, use the corresponding tracker label.
Edit the tracker-label column when the repository's vocabulary changes.
This document defines the mapping; setup does not create remote labels.