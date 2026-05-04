{% if stocks.len() > 0 %}
## 涨停股明细

| 代码 | 名称 | 连板数 | 涨跌幅% | 换手率% | 涨停原因 | 炸板次数 | 流通市值(亿) |
|------|------|--------|---------|---------|----------|----------|--------------|
{% for stock in stocks %}
| {{ stock.symbol }} | {{ stock.name }} | {{ stock.days }}板 | {{ stock.change_percent }} | {{ stock.turnover }} | {{ stock.reason }} | {{ stock.bomb }} | {{ stock.capital }} |
{% endfor %}
{% endif %}