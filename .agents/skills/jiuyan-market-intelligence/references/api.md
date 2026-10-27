# 韭研社区情报接口

动态 token 缺失时，`../scripts/fetch_market_intelligence.py` 会自动调用同目录的
`get_token.mjs`。也可以单独运行：

```bash
node <skill-dir>/scripts/get_token.mjs
```

- Base URL：`https://web-api.jiuyangongshe.com/jystock-app`
- Endpoint：`POST /api/v2/article/community`
- Body：

```json
{
  "category_id": "",
  "limit": 20,
  "order": 1,
  "start": 1,
  "type": 0,
  "back_garden": 0
}
```

必需请求头：

```text
platform: 3
timestamp: <browser timestamp>
token: <browser token>
X-Requested-With: XMLHttpRequest
Content-Type: application/json
Cookie: SESSION=<optional for public content>
```

响应包络：

```json
{"errCode":"0","msg":"","data":{"result":[],"hasNext":true}}
```

文章常用字段：

- `article_id`、`title`、`subtitle`、`content`
- `user.nickname`
- `like_count`、`comment_count`、`collect_count`、`forward_count`
- `create_time`
- `stock_list[]`：`stock_id`、`name`、`code`

常见错误：

- `errCode=1`：未登录或 SESSION 失效
- `errCode=110`：动态 timestamp/token 失效
