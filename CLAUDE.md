# CLAUDE.md
## 项目原型设计
./opendesign
## 技术架构
前端 bun, react
后端 rust, axum
## 执行完任务提问是否提交代码
## 不要为了测试用例成功修改测试用例
## 首要原则
正确的设计 > 兼容现有代码

如果最优方案需要修改现有接口，直接提出来。
不要因为"现在这样写了"就将就，技术债越早还越便宜。
## web端和桌面端同步
如果web端有修改要看一下桌面端是否需要同步修改

## 保持项目代码整洁，低耦合

## Anti-Patterns

- Do not add heavy dependencies for minor convenience.
- Do not silently weaken security policy or access constraints.
- Do not add speculative config/feature flags "just in case".
- Do not mix massive formatting-only changes with functional changes.
- Do not modify unrelated modules "while here".
- Do not bypass failing checks without explicit explanation.
- Do not hide behavior-changing side effects in refactor commits.
- Do not suppress unused production code with underscore prefixes or `#[allow(dead_code)]`; delete it, wire it into behavior, or track a follow-up issue. Reserve underscore names for required but intentionally unused API, trait, or callback parameters.
- Do not leave `unwrap()` / `expect()` in production paths; propagate errors or document the invariant that makes panic impossible.
- Do not include personal identity or sensitive information in test data, examples, docs, or commits.
