# 用户 {{ nickname }} 的帖子

共 {{ post_count }} 篇帖子

{% for post in posts %}
- {{ loop.index }}. {{ post.title }} — 时间: {{ post.create_time }} | 浏览: {{ post.view_count }} | 回复: {{ post.reply_count }}
  - 内容: {{ post.content | safe }}
  - 原文链接: {{ post.link }}
{% endfor %}

[返回列表](index.md)
