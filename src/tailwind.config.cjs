module.exports = {
  content: [
    './src/**/*.{vue,js,ts,jsx,tsx}',
    './index.html',
  ],
  theme: {
    extend: {
      colors: {
        canvas: {
          DEFAULT: '#ffffff',
          parchment: '#f5f5f7',
        },
        'surface-dark': {
          DEFAULT: '#272729',
          2: '#2a2a2c',
          3: '#252527',
        },
        ink: {
          DEFAULT: '#1d1d1f',
          muted: '#86868b',
        },
        primary: {
          DEFAULT: '#0066cc',
          focus: '#0071e3',
          'on-dark': '#2997ff',
        },
        'data-green': '#34c759',
        'data-red': '#ff3b30',
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
        hairline: 'rgba(0, 0, 0, 0.08)',
        'hairline-dark': 'rgba(255, 255, 255, 0.06)',
      },
    },
  },
  plugins: [],
}
