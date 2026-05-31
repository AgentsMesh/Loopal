---
name: Cold Email Templates
description: Reusable templates for outbound email
type: project
created_at: 2026-03-25
updated_at: 2026-05-02
ttl_days: 90
related:
  - cold-email-deliverability
  - twitter-dm-outreach
---

Templates live in templates/cold/*.j2 with merge fields {name}, {company}, {pain_point}. Subject lines must vary per template; deliverability concerns in [[cold-email-deliverability]] kill domain reputation if subjects collide. Adjacent channel: [[twitter-dm-outreach]] shares the same merge-field convention.
