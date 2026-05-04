## 连板高度分布

{% for (boards, count) in distribution %}
- {{ boards }}板：{{ count }}家
{% endfor %}