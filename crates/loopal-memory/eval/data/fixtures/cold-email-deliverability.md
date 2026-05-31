---
name: Cold Email Deliverability
description: SPF/DKIM/DMARC + warmup procedure
type: project
created_at: 2026-03-27
updated_at: 2026-04-30
ttl_days: 90
related:
  - cold-email-templates
---

Domain warmup: send 5/day to known-engaged contacts for week 1, ramp to 50/day by week 4. SPF/DKIM/DMARC must align on the sending domain. Template variations from [[cold-email-templates]] protect against subject-line collision blacklisting.
