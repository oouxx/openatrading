{% if plates.len() > 0 %}
## 韭研公社异动

{% for plate in plates %}
### {{ plate.name }}（{{ plate.count }}只）

{% if plate.reason != "" %}
> {{ plate.reason }}
{% endif %}

{% if plate.stocks.len() > 0 %}
| 时间 | 代码 | 名称 | 连板 | 异动理由 |
|------|------|------|------|----------|
{% for stock in plate.stocks %}
| {{ stock.time }} | {{ stock.code }} | {{ stock.name }} | {{ stock.num }} | {{ stock.desc }} |
{% endfor %}
{% endif %}
{% endfor %}
{% endif %}