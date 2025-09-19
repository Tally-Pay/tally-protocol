# Tally Web3 Frontend

Modern HTMX-powered frontend for the Tally subscription platform - a Blink-native subscription engine for Solana.

## Overview

The Tally Web3 frontend provides a merchant dashboard for managing subscription plans, monitoring customer subscriptions, and viewing analytics. Built with HTMX for seamless server-side rendering, Basecoat UI components for consistent design, and Tailwind CSS for modern styling.

### Key Features

- **Merchant Dashboard**: Comprehensive subscription and plan management
- **Real-time Updates**: HTMX-powered dynamic content loading
- **Responsive Design**: Mobile-first approach with full accessibility support
- **Modern UI**: Basecoat UI components with Tally branding
- **Performance Optimized**: Minimal JavaScript footprint, fast loading times
- **Developer Friendly**: Hot reloading, TypeScript support, modern tooling

## Tech Stack

- **HTMX** - Dynamic content loading and server-side rendering
- **Basecoat UI** - Consistent, accessible component system
- **Tailwind CSS v3.4** - Utility-first CSS framework
- **Alpine.js** - Lightweight client-side interactivity
- **Vite** - Modern build tool and development server
- **Askama** - Rust templating (backend integration)

## Quick Start

### Prerequisites

- Node.js 18+ and pnpm
- Running tally-actions backend service
- Access to Solana network (localnet/devnet/mainnet)

### Installation

```bash
# Clone and navigate to the workspace
cd tally-web3-workspace/tally-web3

# Install dependencies
pnpm install

# Start development server
pnpm dev
```

The development server will start at `http://localhost:5173` with hot reloading enabled.

### Development Workflow

1. **Start Backend**: Ensure the tally-actions service is running
2. **Start Frontend**: Run `pnpm dev` for development server
3. **Make Changes**: Edit templates, styles, or JavaScript
4. **Test**: Verify changes in browser with responsive design testing
5. **Validate**: Run linting and formatting before committing

## Project Structure

```
tally-web3/
├── static/                     # Static assets
│   ├── css/
│   │   ├── input.css          # Tailwind CSS source
│   │   └── styles.css         # Generated CSS bundle
│   ├── js/
│   │   └── app.js             # Main application JavaScript
│   └── favicon.ico            # Site icon
├── templates/                  # Askama templates
│   ├── partials/
│   │   ├── fragments/         # HTMX fragments
│   │   └── header.html        # Shared header component
│   ├── index.html             # Landing page
│   ├── overview.html          # Dashboard overview
│   ├── plans.html             # Subscription plans
│   ├── subscriptions.html     # Customer subscriptions
│   ├── analytics.html         # Analytics dashboard
│   └── settings.html          # Account settings
├── dist/                      # Build output
├── node_modules/              # Dependencies
├── package.json               # Project configuration
├── tailwind.config.js         # Tailwind configuration
├── vite.config.js            # Vite build configuration
├── eslint.config.js          # ESLint configuration
└── prettier.config.js        # Prettier configuration
```

## Build & Deployment

### Development Build

```bash
# Build CSS
pnpm build:css

# Build all assets
pnpm build

# Start development server
pnpm dev
```

### Production Build

```bash
# Production build with optimization
pnpm build:prod

# Preview production build
pnpm preview
```

### Build Outputs

- **CSS**: `dist/css/styles.css` (~36KB, ~7KB gzipped)
- **JavaScript**: `static/js/app.js` (~16KB, ~3.6KB gzipped)
- **Templates**: Served by tally-actions backend

## Integration with tally-actions

The frontend integrates with the tally-actions Rust backend service:

### API Endpoints

- **Static Assets**: Served from `/static/` path
- **Templates**: Rendered by Askama template engine
- **HTMX Fragments**: Dynamic content from `/x/` endpoints
- **Authentication**: Wallet-based auth with session management

### Template Integration

Templates use Askama (Jinja2-style) syntax compatible with the Rust backend:

```html
<!-- Example template syntax -->
{% for plan in plans %}
  <div class="card">
    <h3>{{ plan.name }}</h3>
    <p>{{ plan.price }} USDC/{{ plan.period }}</p>
  </div>
{% endfor %}
```

### HTMX Fragments

Dynamic content is loaded via HTMX fragments:

```html
<!-- Triggers request to /x/plans/table -->
<div hx-get="/x/plans/table" hx-trigger="load" hx-swap="innerHTML">
  <!-- Loading skeleton -->
</div>
```

## Configuration

### Environment Variables

Create `.env.local` for local development:

```bash
# Frontend configuration
PORT=5173
PUBLIC_API_URL=http://localhost:8787

# Backend integration
BACKEND_URL=http://localhost:8787
```

### Tailwind Configuration

Customized for Tally branding in `tailwind.config.js`:

```javascript
module.exports = {
  content: ['./templates/**/*.html', './static/**/*.js'],
  theme: {
    extend: {
      colors: {
        primary: {
          DEFAULT: '#6366f1',
          foreground: '#ffffff',
        },
        // ... Tally brand colors
      },
    },
  },
}
```

## Development Guidelines

### Code Quality

```bash
# Linting and formatting
pnpm lint          # ESLint check
pnpm format        # Prettier formatting
pnpm type-check    # TypeScript validation
```

### Testing

```bash
# Accessibility testing
pnpm test:a11y

# Visual regression testing
pnpm test:visual

# Performance testing
pnpm test:perf
```

### Responsive Design

Test across breakpoints:
- **Mobile**: 320px - 767px
- **Tablet**: 768px - 1023px
- **Desktop**: 1024px - 1439px
- **Large**: 1440px+

### Accessibility

- WCAG 2.1 AA compliance required
- Semantic HTML structure
- Proper ARIA labels and roles
- Keyboard navigation support
- Screen reader compatibility

## Performance Guidelines

### Optimization Targets

- **CSS Bundle**: <50KB uncompressed, <10KB gzipped
- **JavaScript**: <20KB uncompressed, <5KB gzipped
- **First Contentful Paint**: <1.5s
- **Largest Contentful Paint**: <2.5s
- **Cumulative Layout Shift**: <0.1

### Best Practices

- Use Basecoat UI components for consistency
- Minimize custom CSS and JavaScript
- Leverage HTMX for dynamic content
- Optimize images and assets
- Enable gzip compression

## Browser Support

- **Modern Browsers**: Chrome 90+, Firefox 88+, Safari 14+, Edge 90+
- **Mobile**: iOS Safari 14+, Chrome Mobile 90+
- **Features**: ES2020, CSS Grid, Flexbox, Custom Properties

## Troubleshooting

### Common Issues

**HTMX endpoints return 404**
- Ensure tally-actions backend is running
- Check endpoint paths match backend routes
- Verify authentication headers

**CSS not updating**
- Run `pnpm build:css` to rebuild Tailwind
- Clear browser cache
- Check for CSS syntax errors

**JavaScript errors**
- Check browser console for error details
- Verify all dependencies are installed
- Ensure Alpine.js and HTMX are loaded

**Template rendering issues**
- Verify Askama template syntax
- Check variable names match backend data
- Ensure proper HTML escaping

### Debug Mode

Enable debug mode in `static/js/app.js`:

```javascript
TallyApp.config.enableDebugMode = true
```

This enables:
- Console logging for HTMX events
- Request/response debugging
- Performance monitoring
- Error tracking

### Performance Debugging

```bash
# Bundle analysis
pnpm analyze

# Lighthouse audit
pnpm audit:lighthouse

# Memory usage check
pnpm audit:memory
```

## Contributing

1. **Setup**: Follow quick start instructions
2. **Branch**: Create feature branch from main
3. **Develop**: Make changes following code quality guidelines
4. **Test**: Verify responsive design and accessibility
5. **Format**: Run `pnpm format` and `pnpm lint`
6. **Commit**: Use conventional commit messages
7. **PR**: Submit pull request with description

### Code Standards

- Use Basecoat UI components when possible
- Follow Tailwind utility-first approach
- Maintain accessibility standards
- Write semantic HTML
- Keep JavaScript minimal and functional

## Deployment

### Production Checklist

- [ ] Run production build (`pnpm build:prod`)
- [ ] Verify all assets are optimized
- [ ] Test across supported browsers
- [ ] Validate accessibility compliance
- [ ] Check performance metrics
- [ ] Confirm backend integration
- [ ] Test responsive design
- [ ] Verify error handling

### Static Asset Deployment

The frontend is designed to be served by the tally-actions backend. Static assets should be available at `/static/` path with proper caching headers.

### CDN Configuration

For optimal performance:
- Enable gzip compression
- Set cache headers for static assets
- Use HTTP/2 for improved loading
- Consider using a CDN for global distribution

---

## Related Documentation

- [Tally Platform PRD](../solana-subscriptions/tally-platform-prd.md)
- [Tally Web3 PRD](../solana-subscriptions/tally-web3-prd.md)
- [Basecoat UI Documentation](https://basecoatui.com/)
- [HTMX Documentation](https://htmx.org/)
- [Tailwind CSS Documentation](https://tailwindcss.com/)

For questions or support, refer to the main Tally documentation or create an issue in the repository.