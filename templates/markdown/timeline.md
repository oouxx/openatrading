{% if timeline_by_date.keys().len() > 0 %}
## 时间线

### 事件分布

```mermaid
gantt
    title 时间线事件
    dateFormat YYYY-MM-DD

{% for item in timeline_items %}
    {{ item.grade_emoji }} {{ item.title }} :crit, {{ item.date }}, 1d
{% endfor %}
```

### 详细事件

{% for (date, items) in timeline_by_date %}
#### {{ date }}

{% for item in items %}
- {{ item.grade_emoji }} **{{ item.title }}**{% if item.tags.len() > 0 %} [{{ item.tags }}]{% endif %}
{% if item.content != "" %}
  - {{ item.content }}
{% endif %}
{% endfor %}
{% endfor %}
{% endif %}