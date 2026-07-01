# K线 lightweight-charts React组件 + SSE流 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将旧版 Vue 的 lightweight-charts K线组件移植为 React，后端 `/api/kline/:symbol` 增加 SSE 流端点

**Architecture:**
- 后端新增 `GET /api/kline/:symbol/stream` — 每 3s 轮询，period 变化时重新拉取全量数据推送
- 前端新建 `KlineChart.tsx` React 组件（纯 chart 逻辑），`KlineDrawer.tsx` 保留布局/header/metrics/plates，将 GIF img 替换为 KlineChart
- 在已有 `subscribeRealtime` hook 同一文件增加 `subscribeKlineStream` 导出

**Tech Stack:** lightweight-charts ^5.2.0, React, SSE (axum async_stream)

---
