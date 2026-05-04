{% if events_by_time.keys().len() > 0 %}
## 板块异动

### 涨跌分布

```mermaid
pie title 板块涨跌
    上涨 : {{ rise_count }}
    下跌 : {{ fall_count }}
```

### 按时间排序

{% for (datetime, events) in events_by_time %}
#### {{ datetime }}

{% for event in events %}
##### {{ event.plate_name }}
- 涨跌幅: {{ event.pcp }}

{% if event.related_stocks.len() > 0 %}
- 关联股票:
{% for stock in event.related_stocks %}
  - {{ stock.name }} ({{ stock.pcp }})
{% endfor %}
{% endif %}
{% endfor %}
{% endfor %}
{% endif %}