{% if analyses.len() > 0 %}
{% for analysis in analyses %}
## {{ analysis.role_name }} 视角

{{ analysis.content }}

> 响应耗时：{{ analysis.elapsed_ms }}ms
{% endfor %}
{% endif %}