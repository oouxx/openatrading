import { Component, type ErrorInfo, type ReactNode } from "react"
import { Button } from "@/components/ui/button"

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
        <div className="text-secondary text-body flex h-full flex-col items-center justify-center gap-3 p-6 text-center">
          <p>该区域渲染出错。</p>
          <pre className="bg-hover font-number text-caption max-h-48 w-full max-w-xl overflow-auto rounded-md p-2 text-left">
            {this.state.error?.message ?? "Unknown error"}
          </pre>
          <Button size="sm" onClick={() => this.setState({ hasError: false, error: undefined })}>
            重试
          </Button>
        </div>
      )
    }
    return this.props.children
  }
}
