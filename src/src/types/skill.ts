/** 技能来源：用户目录、项目目录、临时目录（pi-rs `SourceScope`）。 */
export type SkillScope = "user" | "project" | "temporary"

/** `GET /api/skills` 返回的技能，包含 SKILL.md 的 Markdown 正文。 */
export interface SkillEntry {
  name: string
  description: string
  file_path: string
  base_dir: string
  disable_model_invocation: boolean
  scope: SkillScope
  /** frontmatter 之后的 Markdown 正文。 */
  content: string
}

/** 新建 / 更新技能时提交给后端的字段。 */
export interface SkillPayload {
  name: string
  description: string
  content: string
  disableModelInvocation: boolean
}
