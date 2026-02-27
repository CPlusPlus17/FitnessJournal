import { NextRequest, NextResponse } from 'next/server';

const ALLOWED_PATHS = new Set([
  'progression',
  'recovery',
  'workouts/today',
  'workouts/upcoming',
  'muscle_heatmap',
  'chat',
  'generate',
  'profiles',
  'analyze',
  'force-pull',
]);

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

function backendApiBaseUrl(): string {
  return (process.env.FITNESS_API_BASE_URL || 'http://fitness-api:3001').replace(/\/+$/, '');
}

async function forwardRequest(
  request: NextRequest,
  method: 'GET' | 'POST' | 'PUT',
  pathSegments: string[],
): Promise<NextResponse> {
  const path = pathSegments.join('/');
  if (!ALLOWED_PATHS.has(path)) {
    return NextResponse.json(
      { status: 'error', message: 'Unknown API route' },
      { status: 404 },
    );
  }

  const headers = new Headers();
  const token = process.env.FITNESS_API_TOKEN || process.env.API_AUTH_TOKEN;
  if (token) {
    headers.set('x-api-token', token);
  }

  const contentType = request.headers.get('content-type');
  if (contentType) {
    headers.set('content-type', contentType);
  }

  let body: string | undefined;
  if (method === 'POST' || method === 'PUT') {
    const rawBody = await request.text();
    if (rawBody) {
      body = rawBody;
    }
  }

  const targetUrl = `${backendApiBaseUrl()}/api/${path}`;

  try {
    const response = await fetch(targetUrl, {
      method,
      headers,
      body,
      cache: 'no-store',
    });

    const responseBody = await response.text();
    const responseContentType =
      response.headers.get('content-type') || 'application/json; charset=utf-8';

    return new NextResponse(responseBody, {
      status: response.status,
      headers: {
        'content-type': responseContentType,
      },
    });
  } catch {
    return NextResponse.json(
      { status: 'error', message: 'Failed to reach backend API' },
      { status: 502 },
    );
  }
}

type RouteParams = {
  path: string[];
};

export async function GET(
  request: NextRequest,
  context: { params: Promise<RouteParams> },
): Promise<NextResponse> {
  const { path } = await context.params;
  return forwardRequest(request, 'GET', path || []);
}

export async function POST(
  request: NextRequest,
  context: { params: Promise<RouteParams> },
): Promise<NextResponse> {
  const { path } = await context.params;
  return forwardRequest(request, 'POST', path || []);
}

export async function PUT(
  request: NextRequest,
  context: { params: Promise<RouteParams> },
): Promise<NextResponse> {
  const { path } = await context.params;
  return forwardRequest(request, 'PUT', path || []);
}
