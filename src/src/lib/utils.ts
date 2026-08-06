import { clsx, type ClassValue } from "clsx"
import { extendTailwindMerge } from "tailwind-merge"

// 应用自定义字号 token（styles/theme.css --font-size-*）。tailwind-merge 默认
// 把 `text-<任意值>` 当作颜色（color: [isAny]），导致 `text-caption` 等自定义
// 字号在与 `text-brand` / `text-down` 等颜色类合并时被当作颜色互相冲突、后者被
// 丢弃。这里把它们显式注册为字号，避免冲突。
const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      "font-size": [{ text: ["caption", "body", "h1", "h2", "hero"] }],
    },
  },
})

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
