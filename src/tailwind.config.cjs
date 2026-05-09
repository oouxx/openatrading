module.exports = {
  content: [
    './src/**/*.{vue,js,ts,jsx,tsx}',
    './index.html',
  ],
  theme: {
    extend: {
      colors: {
        // Backgrounds
        canvas: {
          DEFAULT: '#0d0d1a',
          parchment: '#111124',
        },
        'surface-dark': {
          DEFAULT: '#111124',
          2: '#1a1a35',
          3: '#0f0f20',
        },
        // Text
        ink: {
          DEFAULT: '#e8e8ed',
          muted: '#888780',
          dim: 'rgba(255,255,255,0.06)',
        },
        // Semantic: A-share 红涨绿跌
        up: {
          DEFAULT: '#FF5C5C',
          muted: 'rgba(255,92,92,0.15)',
        },
        down: {
          DEFAULT: '#1D9E75',
          muted: 'rgba(29,158,117,0.15)',
        },
        // AI purple
        ai: {
          DEFAULT: '#7F77DD',
          focus: '#9990f0',
          muted: 'rgba(127,119,221,0.15)',
        },
        // Legacy aliases (keep for gradual migration)
        primary: {
          DEFAULT: '#7F77DD',
          focus: '#9990f0',
          'on-dark': '#7F77DD',
        },
        'data-green': '#1D9E75',
        'data-red': '#FF5C5C',
        bg: {
          DEFAULT: '#0d0d1a',
          surface: '#111124',
          hover: '#1a1a35',
        },
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
        hairline: 'rgba(255,255,255,0.06)',
        'hairline-dark': 'rgba(255,255,255,0.04)',
      },
      boxShadow: {
        terminal: '0 1px 3px rgba(0,0,0,0.3)',
        'terminal-lg': '0 4px 12px rgba(0,0,0,0.4)',
      },
    },
  },
  plugins: [],
}
