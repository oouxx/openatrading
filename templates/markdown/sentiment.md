{% if trend.len() > 0 %}
## 社群舆情分析

**热度趋势**：{{ trend }}

### 热门股票（提及次数）

{% for stock in hot_stocks %}
- {{ stock.1 }} ({{ stock.0 }}) - {{ stock.2 }}次
{% endfor %}

### 热门文章（热度）

{% for article in hot_articles %}
- {{ article.0 }} (热度: {{ article.1 }})
{% endfor %}

### 今日新闻资讯摘要

{{ news_summary }}

**总结**：{{ summary }}

{% endif %}
