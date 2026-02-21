import { NextRequest, NextResponse } from 'next/server';

const DEFAULT_ADMIN_USER = 'admin';
const AUTH_REALM = 'Fitness Dashboard Settings';

function isProtectedPath(pathname: string): boolean {
  return pathname.startsWith('/settings') || pathname === '/api/profiles';
}

function parseBasicAuth(authHeader: string): { username: string; password: string } | null {
  if (!authHeader.startsWith('Basic ')) {
    return null;
  }

  const encodedCredentials = authHeader.slice('Basic '.length).trim();
  if (!encodedCredentials) {
    return null;
  }

  try {
    const decoded = atob(encodedCredentials);
    const separator = decoded.indexOf(':');
    if (separator < 0) {
      return null;
    }

    return {
      username: decoded.slice(0, separator),
      password: decoded.slice(separator + 1),
    };
  } catch {
    return null;
  }
}

function unauthorizedResponse(): NextResponse {
  return new NextResponse('Authentication required', {
    status: 401,
    headers: {
      'www-authenticate': `Basic realm="${AUTH_REALM}"`,
    },
  });
}

export function middleware(request: NextRequest): NextResponse {
  if (!isProtectedPath(request.nextUrl.pathname)) {
    return NextResponse.next();
  }

  const adminPassword =
    process.env.DASHBOARD_ADMIN_PASSWORD ||
    process.env.FITNESS_API_TOKEN ||
    process.env.API_AUTH_TOKEN;
  if (!adminPassword) {
    return new NextResponse(
      'Dashboard admin authentication is not configured. Set DASHBOARD_ADMIN_PASSWORD (or API_AUTH_TOKEN).',
      { status: 503 },
    );
  }

  const adminUsername = process.env.DASHBOARD_ADMIN_USERNAME || DEFAULT_ADMIN_USER;
  const authHeader = request.headers.get('authorization');
  if (!authHeader) {
    return unauthorizedResponse();
  }

  const credentials = parseBasicAuth(authHeader);
  if (!credentials) {
    return unauthorizedResponse();
  }

  if (credentials.username !== adminUsername || credentials.password !== adminPassword) {
    return unauthorizedResponse();
  }

  return NextResponse.next();
}

export const config = {
  matcher: ['/settings/:path*', '/api/profiles'],
};
