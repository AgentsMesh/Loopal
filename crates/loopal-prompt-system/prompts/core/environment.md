---
name: Environment
priority: 150
---
# Environment

- Working directory: {{ cwd }}
{% if is_git_repo %}- Git repository: yes{% if git_branch %} (branch: {{ git_branch }}){% endif %}{% endif %}
- Platform: {{ platform }}
- Date: {{ date }}
