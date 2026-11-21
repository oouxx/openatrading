# 附件与缓存

## 设计：把「读文件」交给 pi-rs

附件不在本应用里做解析。除了两处例外，prompt 里放的就是**磁盘上的绝对路径**，
由 pi-rs 内置的 `read` 工具按需读取（2000 行 / 50KB 一次，`offset` 续读）：

```xml
<openatrading_attachments marker="openatrading-v1">
  <file name="/home/u/.atrading/attachments/s1/8f3-年报.pdf" size="4820000"/>
  <note>Files without inline content are on disk. Read them with the read tool;
  use offset/limit to continue a long file.</note>
</openatrading_attachments>
```

两个例外：

| 例外 | 原因 |
|---|---|
| **图片** → `ContentBlock::Image` | 必须解码才能给模型看，没法懒加载。归一化用的是 pi-rs 的 `process_image`（嗅探格式、必要时转 PNG、按模型内联上限缩放） |
| **≤64KB 的 UTF-8 文本** → 内联进 prompt | 省掉一次工具往返。大于 64KB、非 UTF-8、含控制字符的一律只引用路径 |

由此得到几个直接结果：

- PDF / Word / Excel / PPT **不需要**本应用解析，一份 300MB 的日志和一份 2MB 的 PDF
  走的是同一条路；
- 没有「文本太长被截断」这回事，也没有 1MB 上限；
- 附件内容不常驻消息历史，之后每一轮对话都不会重发全文。

## 上限

| 项 | 值 | 在哪 |
|---|---|---|
| 单条消息附件数 | 10（前端）/ 20（后端硬上限） | `ChatView.tsx` / `attachments.rs` |
| 附件字节数 | **不限** | —— |
| 内联文本阈值 | 64KB | `INLINE_MAX_TEXT_BYTES` |
| 单张图片 | 64MB（需要解码，必须有上限） | `MAX_IMAGE_BYTES` |
| 一次上传请求 | 2GB（只为挡住拖错几十 GB 的镜像） | `attachments_api.rs` |

## 文件存在哪

```
~/.atrading/attachments/<会话 id>/<uuid>-<原始文件名>
```

- **Web 端**：浏览器没有路径，选中的文件先 `POST /api/attachments` 落盘，换回
  服务器路径再引用。落盘发生在**发送时**，选完不发不会留垃圾。
- **桌面端**：拖拽/文件选择器本来就有绝对路径，直接引用，字节完全不经过 IPC。
  只有粘贴（没有路径）的字节才落盘。
- 文件名保留中文（模型靠它判断读的是什么），只剥掉路径分隔符与控制字符。
- 删会话时该会话的附件目录一并删除。

## 缓存

「设置 → 缓存」展示并清理三类可重新生成的数据：

| 类别 | 内容 |
|---|---|
| 行情接口缓存 | 进程内内存缓存，重启即失效 |
| 附件缓存 | `~/.atrading/attachments/` |
| 临时文件 | `~/.atrading/tmp/` |

会话记录、角色、模拟盘数据**不在清理范围**。正在被某轮对话读取的附件会被跳过
（结果里注明「正在使用中，已跳过」）。

## 常见问题

**「附件 xxx 不存在或已失效」**
文件被删了（清了缓存、删了会话，或手工移动过）。重新添加一次即可。

**「不在允许读取的目录内」**
仅 Web 端会出现：prompt 只允许引用附件目录下的路径。浏览器的用户不是这台机器的
主人，不该凭一条消息读到服务器上的任意文件。桌面端没有这个限制。

**附件太大 / 发不出去**
没有字节上限。如果 Web 端上传很慢，进度条会显示百分比，可以随时取消。

**图片变糊**
2000px / 4.5MB 是模型侧的内联上限（OpenAI 2048px、Claude 1568px 起），超出由
pi-rs 自动缩放。要看细节请先裁剪。

**桌面端拖入的图片没有缩略图**
拖拽走的是路径直读，不把整张图读进内存，所以不做预览；文件名与大小照常显示。
