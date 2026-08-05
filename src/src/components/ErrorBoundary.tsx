import { Component, type ErrorInfo, type ReactNode } from "react"

interface Props {
  children: ReactNode
  /** Optional label shown in the fallback, e.g. the wrapped component name. */
  name?: string
}
interface State {
  hasError: boolean
  error?: Error
}

/**
 * Lightweight error boundary that prevents a single component crash from
 * white-screening the whole app. Used to wrap the chat surface (`AiCopilot`),
 * which renders untrusted agent/tool payloads and can otherwise throw on
 * unexpected event shapes.
 */
export default class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(`[ErrorBoundary${this.props.name ? `:${this.props.name}` : ""}]`, error, info)
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="text-secondary flex h-full flex-col items-center justify-center gap-3 p-6 text-center text-body">
          <p>该区域渲染出错。</p>
          <pre className="bg-hover max-h-48 w-full max-w-xl overflow-auto rounded-md p-2 text-left font-mono text-caption">
            {this.state.error?.message ?? "Unknown error"}
          </pre>
          <button
            className="bg-brand hover:bg-brand/90 rounded-md px-3 py-2 text-caption font-medium text-primary transition-colors"
            onClick={() => this.setState({ hasError: false, error: undefined })}
          >
            重试
          </button>
        </div>
      )
    }
    return this.props.children
  }
}
