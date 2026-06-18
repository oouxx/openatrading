module.exports = {
  content: [
    './src/**/*.{js,ts,jsx,tsx}',
    './index.html',
  ],
  theme: {
    extend: {
      colors: {
        canvas: {
          DEFAULT: 'rgb(var(--color-bg-rgb) / <alpha-value>)',
          parchment: 'rgb(var(--color-surface-rgb) / <alpha-value>)',
        },
        'surface-dark': {
          DEFAULT: 'rgb(var(--color-surface-rgb) / <alpha-value>)',
          2: 'rgb(var(--color-surface-hover-rgb) / <alpha-value>)',
          3: 'rgb(var(--color-bg-rgb) / <alpha-value>)',
        },
        ink: {
          DEFAULT: 'rgb(var(--color-fg-rgb) / <alpha-value>)',
          muted: 'rgb(var(--color-muted-rgb) / <alpha-value>)',
          dim: 'rgb(var(--color-fg-rgb) / 0.06)',
        },
        up: {
          DEFAULT: 'rgb(var(--color-up-rgb) / <alpha-value>)',
          muted: 'rgb(var(--color-up-rgb) / 0.15)',
        },
        down: {
          DEFAULT: 'rgb(var(--color-down-rgb) / <alpha-value>)',
          muted: 'rgb(var(--color-down-rgb) / 0.15)',
        },
        ai: {
          DEFAULT: 'rgb(var(--color-accent-rgb) / <alpha-value>)',
          focus: 'rgb(var(--color-accent-hover-rgb) / <alpha-value>)',
          muted: 'rgb(var(--color-accent-rgb) / 0.15)',
        },
        primary: {
          DEFAULT: 'rgb(var(--color-accent-rgb) / <alpha-value>)',
          foreground: 'hsl(var(--primary-foreground))',
          focus: 'rgb(var(--color-accent-hover-rgb) / <alpha-value>)',
          'on-dark': '#7F77DD',
        },
        'data-green': 'rgb(var(--color-down-rgb) / <alpha-value>)',
        'data-red': 'rgb(var(--color-up-rgb) / <alpha-value>)',
        bg: {
          DEFAULT: 'rgb(var(--color-bg-rgb) / <alpha-value>)',
          surface: 'rgb(var(--color-surface-rgb) / <alpha-value>)',
          hover: 'rgb(var(--color-surface-hover-rgb) / <alpha-value>)',
        },
        white: 'rgb(var(--color-fg-rgb) / <alpha-value>)',
        black: 'rgb(var(--color-bg-rgb) / <alpha-value>)',
        /* shadcn standard tokens — used by @apply in style.css and generated components */
        background: 'oklch(var(--background) / <alpha-value>)',
        foreground: 'oklch(var(--foreground) / <alpha-value>)',
        card: {
          DEFAULT: 'oklch(var(--card) / <alpha-value>)',
          foreground: 'oklch(var(--card-foreground) / <alpha-value>)',
        },
        popover: {
          DEFAULT: 'oklch(var(--popover) / <alpha-value>)',
          foreground: 'oklch(var(--popover-foreground) / <alpha-value>)',
        },
        secondary: {
          DEFAULT: 'oklch(var(--secondary) / <alpha-value>)',
          foreground: 'oklch(var(--secondary-foreground) / <alpha-value>)',
        },
        muted: {
          DEFAULT: 'oklch(var(--muted) / <alpha-value>)',
          foreground: 'oklch(var(--muted-foreground) / <alpha-value>)',
        },
        accent: {
          DEFAULT: 'oklch(var(--accent) / <alpha-value>)',
          foreground: 'oklch(var(--accent-foreground) / <alpha-value>)',
        },
        destructive: {
          DEFAULT: 'oklch(var(--destructive) / <alpha-value>)',
        },
        border: 'oklch(var(--border))',
        input: 'oklch(var(--input))',
        ring: 'oklch(var(--ring) / <alpha-value>)',
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', '-apple-system', 'BlinkMacSystemFont', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      fontSize: {
        'display-lg': ['40px', { lineHeight: '1.1', letterSpacing: '-0.5px', fontWeight: '600' }],
        'display-md': ['28px', { lineHeight: '1.14', letterSpacing: '-0.3px', fontWeight: '600' }],
        body: ['17px', { lineHeight: '1.47', letterSpacing: '-0.3px' }],
        'body-strong': ['17px', { lineHeight: '1.24', letterSpacing: '-0.3px', fontWeight: '600' }],
        caption: ['14px', { lineHeight: '1.43', letterSpacing: '-0.2px' }],
        'caption-strong': ['14px', { lineHeight: '1.29', letterSpacing: '-0.2px', fontWeight: '600' }],
        'nav-link': ['12px', { lineHeight: '1.0', letterSpacing: '-0.1px' }],
      },
      borderRadius: {
        card: '18px',
        button: '9999px',
        input: '18px',
        badge: '5px',
      },
      spacing: {
        'section-y': '80px',
        'card-inner': '24px',
      },
      borderColor: {
        hairline: 'rgb(var(--color-border-rgb) / 0.06)',
        'hairline-dark': 'rgb(var(--color-border-rgb) / 0.04)',
      },
      boxShadow: {
        terminal: '0 1px 3px rgba(0,0,0,0.3)',
        'terminal-lg': '0 4px 12px rgba(0,0,0,0.4)',
      },
    },
  },
  plugins: [],
}
