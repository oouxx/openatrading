/// <reference types="vite/client" />

declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  const component: DefineComponent<object, object, unknown>
  export default component
}

declare const __OPENROUTER_KEY__: string
declare const __JIUYAN_PHONE__: string
declare const __JIUYAN_PASSWORD__: string
declare const __API_BASE_URL__: string
