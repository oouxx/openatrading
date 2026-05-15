# cli
cli的作用是提供一个命令行接口，用于与trending-agent进行交互。把trending-agent的功能封装成一个命令行工具，方便用户在终端中使用。
同时支持作为skill插件在openclaw, hermes agent中使用。

## 输出格式
输出格式包括json, markdown, html 三种格式。
1. json格式数据使用`--json`参数，默认输出到stdout。
2. markdown格式数据使用`--markdown`参数，默认输出到stdout。
3. html格式数据使用`--html`参数，默认输出到stdout。

同时可以指定输出文件，使用`--output`参数。
json格式数据直接序列化得到，markdown和html使用模版渲染。

可以通过命令行参数--data=limit_up,limit_down,super_stock,field_item 指定将哪些数据输出到stdout或者文件中。例如
