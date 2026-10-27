# 韭研板块异动接口

动态 token 缺失时，`../scripts/fetch_sector_movements.py` 会自动调用同目录的
`get_token.mjs`。也可以单独运行：

```bash
node <skill-dir>/scripts/get_token.mjs
```

- Base URL：`https://web-api.jiuyangongshe.com/jystock-app`
- Endpoint：`POST /api/v1/action/field`
- Body：

```json
{"date":"2026-10-27","pc":1}
```

必需请求头：

```text
platform: 3
timestamp: <browser timestamp>
token: <browser token>
Cookie: SESSION=<session token>
X-Requested-With: XMLHttpRequest
Content-Type: application/json
```

响应包络：

```json
{"errCode":"0","msg":"","data":[]}
```

常见错误：

- `errCode=1`：未登录或 SESSION 失效
- `errCode=110`：动态 timestamp/token 失效

`data[]` 常见字段：

- `name`、`reason`、`count`
- `list[]`：`code`、`name`、`article.action_info`
- `action_info` 可能包含 `time`、`num`、`expound`、`price`、`day`、`edition`、`shares_range`
