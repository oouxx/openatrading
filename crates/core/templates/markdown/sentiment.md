{% if trend.len() > 0 %}
## 社群舆情分析

**热度趋势**：{{ trend }}

### 热门股票（提及次数）

{% for stock in hot_stocks %}
- {{ stock.1 }} ({{ stock.0 }}) - {{ stock.2 }}次
{% endfor %}

### 热门文章（热度）

{% for article in hot_articles %}
- [{{ article.title }}](https://www.jiuyangongshe.com/a/{{ article.article_id }}) — {{ article.nickname }} | 👍{{ article.like_count }} 💬{{ article.comment_count }} 🔄{{ article.forward_count }} (热度: {{ article.heat_score }})
{% endfor %}

### 今日新闻资讯摘要

{{ news_summary }}

**总结**：{{ summary }}

### 社群文章列表（{{ articles.len() }}篇）

{% for article in articles %}
<details>
<summary><strong>{{ article.title }}</strong> - {{ article.nickname }} | {{ article.create_time }} | 👍 {{ article.like_count }} 💬 {{ article.comment_count }} 🔄 {{ article.forward_count }} ⭐ {{ article.collect_count }}</summary>

**原文链接**：https://www.jiuyangongshe.com/a/{{ article.article_id }}

{% if article.stocks.len() > 0 %}
**关联股票**：
{% for stock in article.stocks %}
- {{ stock.name }} ({{ stock.code }})
{% endfor %}
{% endif %}

{% if article.content.len() > 0 %}
{{ article.content }}
{% endif %}

</details>

{% endfor %}

{% endif %}
