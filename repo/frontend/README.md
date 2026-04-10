# Motorsport Operations — Frontend

Next.js 14 (App Router disabled; Pages Router) + TypeScript frontend for the Motorsport Operations API.

## Prerequisites

- Node.js 18+
- npm 9+
- The Rust API backend running on `http://localhost:8000` (see root `README.md`)

## Quick Start

```bash
# 1. Install dependencies
cd frontend
npm install

# 2. Configure environment
cp .env.local.example .env.local
# Edit .env.local if the API runs on a different host/port

# 3. Start the dev server (hot reload)
npm run dev
# → http://localhost:3000
```

## Available Scripts

| Command | Description |
|---|---|
| `npm run dev` | Start development server with hot reload |
| `npm run build` | Compile and optimise for production |
| `npm run start` | Serve the production build (run `build` first) |
| `npm test` | Run Jest test suite |
| `npm run test:watch` | Run tests in interactive watch mode |
| `npm run test:coverage` | Collect coverage report |

## Project Structure

```
src/
├── pages/              # Next.js page routes
│   ├── _app.tsx        # Global providers (Auth, Toast)
│   ├── index.tsx       # Root redirect → /dashboard or /login
│   ├── login.tsx       # Authentication screen
│   ├── dashboard.tsx   # Overview + role-based navigation cards
│   ├── assets.tsx      # Asset Register — search, filter, paginated table
│   └── results.tsx     # Events & Results — search, filter, paginated table
├── components/
│   ├── auth/           # LoginForm (loading, error, success states)
│   ├── assets/         # AssetTable, AssetSearch
│   ├── events/         # EventsTable, EventsSearch
│   ├── dashboard/      # RoleNav (role-filtered cards), StatCard
│   ├── layout/         # Layout wrapper + sticky Navbar
│   └── ui/             # Spinner, EmptyState, ErrorBanner, ToastContainer
├── services/           # Typed API fetch wrappers
│   ├── api.ts          # Base fetch + ApiRequestError class
│   ├── auth.service.ts
│   ├── assets.service.ts
│   ├── events.service.ts
│   └── results.service.ts
├── hooks/              # useAssets, useEvents
├── context/            # AuthContext, ToastContext
├── utils/              # token.ts, format.ts
└── types/              # Shared TypeScript interfaces
__tests__/              # Jest + React Testing Library
├── login.test.tsx      # LoginForm — happy path + 401 + network error + submitting state
├── assetTable.test.tsx # AssetTable — rows, badges, pagination boundary cases
├── dashboard.test.tsx  # RoleNav role-gating + EmptyState variants
└── events.test.tsx     # EventsTable, EventsSearch, ErrorBanner
```

## Pages at a Glance

| Route | Description | Auth required |
|---|---|---|
| `/` | Redirects to `/dashboard` or `/login` | — |
| `/login` | Email + password form; loading/error states | No |
| `/dashboard` | Stat cards + role-filtered navigation | Yes |
| `/assets` | Asset Register — search + filter + paginated table | Yes |
| `/results` | Events & Results — search + status filter + paginated table | Yes |

## Authentication

Login at `/login` with a username and password. The API returns a bearer token stored in `sessionStorage` (never in a cookie or `localStorage`). All API calls attach it as `Authorization: Bearer <token>`.

Sessions expire after 30 minutes of API inactivity (enforced server-side).

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `NEXT_PUBLIC_API_BASE_URL` | `http://localhost:8000` | Rust API base URL |

## Running Tests

```bash
npm test
# or for a single file:
npx jest __tests__/login.test.tsx
```
