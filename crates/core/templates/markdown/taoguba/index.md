# 淘股吧用户列表

{% for user in users %}
- [{{ user.nickname }}](/taoguba/user-{{ user.user_id }}.md)  用户ID: {{ user.user_id }} | 帖子数: {{ user.post_count }} | 最后更新: {{ user.last_update }} | 置顶帖: {{ user.top_post_title }}
{% endfor %}
